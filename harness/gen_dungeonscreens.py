"""Emit the original's drawing of its 2D screens in a dungeon, over the
user's saved game, for view/src/screentest.rs.

Usage: gen_dungeonscreens.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR]

SAVE_DIR holds the saved game in slot 1 (the user's: only read), loaded
from the title as gen_screens.py loads it (harness/screens.py). The save
is in Daggerfall's mages' guild and reaches no dungeon, so the way into
one is the outer loop's own, from memory: the guild left for the street
(`area_ended`), then the street left with the next area a dungeon (5E6
3) and the location's dungeon seed (A56) DUNGEON_SEED (a town's record
has none: 0, from which 0000:28BC's picks of the stairs' chunks never
differ and it loops for ever). The outer loop makes the dungeon from it
(2294:0ED4, 0000:28BC) and runs it (0000:09FA) until its world loop is
about to run its first frame (0000:A2AC), the first level built and the
player at its start.

Each screen of SCREENS is then opened as gen_screens.py opens its own
(`draw`: the page and the VGA filled with 00h, then FFh, the pixels the
same in both the ones drawn) and written as gen_screens.py writes them:

  screen | NAME | <page digest> <palette digest> [<mask digest>]

and the colours on the way, when more than one, to dungeonfades.txt
beside OUT:

  screenfade | NAME | <count> | <digest>
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE
from screens import ScreenArena
import gen_screens as gs

# The location's dungeon seed both sides make the dungeon from.
DUNGEON_SEED = 0x2F6E_1A35

SCREENS = {
    # S: the status box (037C).
    'dungeonstatus': (0x037C, []),
    # M: the automap (045A), its loop asking the mouse by 8FC8.
    'dungeonautomap': (0x045A, [], {(0, 0x8FC8)}),
    # The same with the whole level seen (`seen`): its walls, doors and
    # stairs drawn.
    'dungeonmap': (0x045A, [], {(0, 0x8FC8)}),
    # R: camping (01E6): its menu.
    'dungeoncamp': (0x01E6, []),
    # Shift+M: the world map (03BF).
    'dungeontravel': (0x03BF, []),
}

def seen(a):
    """Every square of the level's automap seen nearest (15h): the map's
    128 x 128 bytes in EMS bank 11h at 9030's segment (0000:49D6's)."""
    gs.square(a)
    a.call_near(0, 0x5C8A, bx=0x11)
    a.mu.mem_write(a.lin(a.r16(0x9030), 0), b'\x15' * 0x4000)


BEFORE = {'dungeonautomap': gs.square, 'dungeonmap': seen}


def dungeon(a, base):
    """The snapshot of the game just inside the dungeon (see the module's
    notes)."""
    a.restore(base)
    assert a.area_ended() == ('world',), 'the guild not left'
    a.mu.mem_write(a.lin(a.ds, 0x05E6), b'\x03')
    a.w32(0x0A56, DUNGEON_SEED)
    assert a.area_ended() == ('world',), 'the dungeon not entered'
    return a.snapshot()


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    gs.BEFORE.update(BEFORE)
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    at = dungeon(a, a.snapshot())
    out = open(OUT, 'w', buffering=1)
    fades = open(os.path.join(os.path.dirname(os.path.abspath(OUT)), 'dungeonfades.txt'), 'w', buffering=1)
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    for name, (handler, script, *waits) in SCREENS.items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        seen = []

        def sample(*_):
            if not seen or seen[-1] != bytes(a.dac):
                seen.append(bytes(a.dac))
        hooks = [a.mu.hook_add(UC_HOOK_CODE, sample, None, a.lin(LOAD_SEG, o), a.lin(LOAD_SEG, o)) for o in (0x6BB8, 0x9B77)]
        p0, dac = gs.draw(a, at, name, handler, script, 0x00, *waits)
        for h in hooks:
            a.mu.hook_del(h)
        if len(seen) > 1:
            fades.write(f'screenfade | {name} | {len(seen)} | {fnv(b"".join(struct.pack("<Q", fnv(d)) for d in seen)):x}\n')
            if dump:
                os.makedirs(dump, exist_ok=True)
                open(os.path.join(dump, f'{name}.fade'), 'wb').write(b''.join(seen))
        p1, _ = gs.draw(a, at, name, handler, script, 0xFF, *waits)
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
