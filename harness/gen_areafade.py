"""Emit the colours the original shows as it goes from one area to the
next (view/src/screentest.rs checks the view's against them).

Usage: gen_areafade.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR]

SAVE_DIR holds the saved game in slot 1 (the user's: only read), loaded
from the title as gen_screens.py loads it (harness/screens.py), and the
guild it's in left for the street (`area_ended`; the loaded world loop
wasn't run by an area's own run, 0000:0AA9, so this first way out isn't
one the player's would be). Each case then goes to the next area as
screens.py does, from its start until the next area's world loop is
about to run its first frame (0000:A2AC):

  - tavern: from the street, the town's first tavern's door clicked
    (`enter`: the wall's trigger 0000:43B1, its way in 4118 with the
    screen faded to black, 1194:55B2; then the area left);
  - leavetavern: from the tavern, the way out (C398 bit 80h, as the
    interior's exit 0000:44BD sets it; `area_ended`): the world loop's
    end back in 0AA9 (0AEA..0AFC: the colours faded to black in 12).

And the world's own way back from a screen up over it:

  - automapclose: in the guild (the save as loaded), M's automap
    (2542:045A, 0000:2CFE; its pen at A0h, 64h as gen_screens.py has it)
    left by its exit (2E30, clicked at 100h, B0h): the colours faded to black in 12 (2EC8..2EDC), the
    world drawn and faded in to 999E in 12 (2EF2); from the click on until
    045A returns.

On the way the colours at each wait for the retrace (0000:6BB8, 9B77) are
kept when different from the one before:

  areafade | NAME | <count> | <digest>

the digest the FNV-1a over each state's FNV-1a (8 bytes, little-endian)
in turn, as screenfades.txt has them; with --dump, NAME.fade holds them
(768 bytes each).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE
from screens import ScreenArena

CASES = ['tavern', 'leavetavern', 'automapclose']
MENU = 0x2542
# The town's taverns (far pointer) and their count (gen_screens.py's PLACES).
TAVERNS = (0x1AB5, 0x1387)


def tavern_door(a):
    """The town's first tavern's door square (as gen_screens.py's
    `place`: an entry's square is row << 8 | column, the map's offset
    doubling the column)."""
    at, count = TAVERNS
    assert a.r16(count), 'no tavern in the town'
    sq = a.r16(a.r16(at), seg=a.r16(at + 2))
    return (sq & 0xFF00) | ((sq & 0xFF) << 1)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    out = open(OUT, 'w', buffering=1)
    guild = a.snapshot()
    assert a.area_ended() == ('world',), 'the guild not left'
    at = a.snapshot()
    for name in CASES:
        print(name, file=sys.stderr)
        a.restore(guild if name == 'automapclose' else at)
        door = tavern_door(a) if name == 'tavern' else None
        seen = []

        def sample(*_):
            if name == 'automapclose' and a.script:
                return
            if not seen or seen[-1] != bytes(a.dac):
                seen.append(bytes(a.dac))
        hooks = [a.mu.hook_add(UC_HOOK_CODE, sample, None, a.lin(LOAD_SEG, o), a.lin(LOAD_SEG, o)) for o in (0x6BB8, 0x9B77)]
        try:
            if name == 'automapclose':
                from gen_screens import square
                square(a)
                a.waits = {(0, 0x8FC8)}
                a.script = [('click', 0x100, 0xB0)]
                a.mouse, a.buttons = (160, 100), 0
                r = a.run_screen(MENU, 0x045A, near=True)
                assert r is None and not a.script, f'the automap not left ({r})'
                r = ('world',)
            else:
                r = a.enter(door) if door is not None else a.area_ended()
        finally:
            for h in hooks:
                a.mu.hook_del(h)
        assert r == ('world',), f'{name}: the next area not reached ({r})'
        at = a.snapshot()
        out.write(f'areafade | {name} | {len(seen)} | {fnv(b"".join(struct.pack("<Q", fnv(d)) for d in seen)):x}\n')
        if dump:
            os.makedirs(dump, exist_ok=True)
            open(os.path.join(dump, f'{name}.fade'), 'wb').write(b''.join(seen))
    out.close()


if __name__ == '__main__':
    main()
