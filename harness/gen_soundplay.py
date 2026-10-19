"""The game with its sound on, as ARENA.BAT runs it: worldframes cases
played with the digital sound driver loaded at the program's start.

The driver (SBPDIG.ADV, -IOS220 -IRQS7 -DMAS1) is loaded as the program's
main loads it, before the title's Load (sbdig.py's `start(sound=True)`),
on a Sound Blaster Pro (sbdig.py's `Blaster`): the load then reads the
level's sound files into their EMS banks (23BA:0217, each formatted by
the driver, 85h) and the frames play the level's sounds (23BA:002A: the
driver asked whether one is playing, 7Ch; the sound's sample copied from
its bank to the driver's buffer, played, 7Bh, at its volume, 81h, and
started, 7Dh). Some of gen_worldframes.py's cases are run so (CASES):
footsteps, a door, a fight, a spell.

Each case's lines are gen_worldframes.py's (`track`, `state`,
`stateload`, `stateems`), its `driver` line the calls into the driver
from the load on, its place's way in and its frames, answers and all
(sbdig.py's `calls_line`), and the load's
own: `loadstate` and `loadems` (the state the load with sound on leaves,
as gen_worldframes.py's), `driver | load` (the load's calls).

Usage: gen_soundplay.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [CASE...]
"""
import os
import struct
import sys

from emu import LOAD_SEG, fnv
from sbdig import SoundArena
from unicorn import UC_HOOK_CODE
import gen_worldframes as gw

# The cases run with sound on (gen_worldframes.py's).
CASES = ['playswim', 'playpick', 'playfight', 'playcast']


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    a = SoundArena(EXE, GAME, SAVE)
    gw.rows_watch(a)
    a.start(sound=True)
    out = open(OUT, 'w', buffering=1)
    loaded = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    loaded_ems = b''.join(a.ems_bank(b) for b in range(0x20))
    if dump:
        os.makedirs(dump, exist_ok=True)
        open(os.path.join(dump, 'base.ds'), 'wb').write(loaded)
        open(os.path.join(dump, 'base.ems'), 'wb').write(loaded_ems)
    skip_base = gw.excluded('loadexclude.txt')
    if not only:
        out.write(f'loadstate | {fnv(bytes(0 if i in skip_base else b for i, b in enumerate(loaded))):x}\n')
        out.write(f'loadems | {fnv(loaded_ems):x}\n')
        out.write(a.blaster.calls_line('load'))
    a.blaster.calls = []
    base = a.snapshot()
    skip = gw.excluded()
    skip_load = skip | gw.excluded('stateloadexclude.txt') | gw.excluded('loadexclude.txt')
    places = {}
    for name in CASES:
        if only and name not in only:
            continue
        where, setup, steps = gw.CASES[name]
        if where not in places:
            places[where] = {'dungeon': gw.gd.dungeon, 'crypt': gw.crypt}[where](a, base)
        print(name, file=sys.stderr)

        tracks = []
        gw.run_case(a, places[where], name, setup, steps, tracks)
        for t in tracks:
            out.write(t)
        out.write(a.blaster.calls_line(name))
        after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        out.write(gw.state_line(name, a.pre_state[0], after, skip))
        out.write(gw.state_line(name, loaded, after, skip_load, 'stateload'))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        out.write(gw.state_runs('stateems', name, a.pre_state[1], ems))
        if dump:
            open(os.path.join(dump, f'{name}.ds'), 'wb').write(after)
            open(os.path.join(dump, f'{name}.ems'), 'wb').write(ems)
            open(os.path.join(dump, f'{name}.ds0'), 'wb').write(a.pre_state[0])
            open(os.path.join(dump, f'{name}.ems0'), 'wb').write(a.pre_state[1])
    out.close()


if __name__ == '__main__':
    main()
