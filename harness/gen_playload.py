"""Emit the original's state after a saved game is loaded in play, for
view/src/worldtest.rs: slot 1 loaded (25FD:028D with AX 1, 5C8 bit 4
clear: the load menu's pick, 2542:061E's way in) at the top of a world
frame in a place, until it returns to the menu's caller.

Usage: gen_playload.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR]

The places, as gen_worldframes.py makes them: `guild`, the game as the
title's Load leaves it (its first frame not run), and `street`, the guild
left for Daggerfall's street. The sounds and the location record's build
are stubbed as for the title's Load (screens.py `stub`).

For each place, DGROUP as the load leaves it (the offsets
tests/vectors/loadexclude.txt names zeroed; the port's in the harness's
form, its image's segments 1000h higher: worldtest's `load_canonical`),
and the EMS banks 0..1Fh (64K each, end to end):

  playload | NAME | <DGROUP digest> <EMS digest>

With --dump DIR, NAME.ds and NAME.ems (the state as the load leaves it)
for worldtest's `the_play_loads_measured`.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import STUB_SEG, fnv
from screens import ScreenArena
import gen_worldframes as gw


def play_load(a, at):
    """Slot 1 loaded in play from the snapshot `at`: DGROUP and the EMS
    banks after."""
    a.restore(at)
    saved = a.stub()
    try:
        a._setup([0, STUB_SEG], {'eax': 1})
        a._run(0x25FD, 0x028D, STUB_SEG, 0, count=500_000_000)
    finally:
        a.unstub(saved)
    # The stubs' counts at the top of DGROUP cleared, as `start` does.
    for at in range(0xFFE0, 0x10000, 2):
        a.w16(at, 0)
    ds = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    ems = b''.join(a.ems_bank(b) for b in range(0x20))
    return ds, ems


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    places = {'guild': base, 'street': gw.street(a, base)}
    skip = gw.excluded('loadexclude.txt')
    out = open(OUT, 'w', buffering=1)
    for name, at in places.items():
        print(name, file=sys.stderr)
        ds, ems = play_load(a, at)
        if dump:
            os.makedirs(dump, exist_ok=True)
            open(os.path.join(dump, name + '.ds'), 'wb').write(ds)
            open(os.path.join(dump, name + '.ems'), 'wb').write(ems)
        canon = bytes(0 if i in skip else b for i, b in enumerate(ds))
        out.write(f'playload | {name} | {fnv(canon):x} {fnv(ems):x}\n')


if __name__ == '__main__':
    main()
