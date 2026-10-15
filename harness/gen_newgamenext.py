"""Emit test vectors for the end of a new game's start dungeon
(src/transit.rs `new_game_next`, 0000:0C98..0CA9 with 0CAD) from the
original code.

Usage: gen_newgamenext.py ACD_unpacked.EXE OUT

Each case is set up as gen_transit.py's (its `setup`, stubs and stops),
then runs from 0000:0C98 with 0000:FFF0 on the stack (0CAC's `ret`
returns there: a stop for leaving, number 6); the next area's run is a
stop recording its number (FFD0) and SI (FFD2).

  newgamenext | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes, far pointers' segments image-relative.
"""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, STUB_SEG
from emu import changes, mine
from gen_attack import fnv
from gen_transit import FAR, STOPS, STUBS, WORLD, make, relocated, setup


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = make(EXE, STUBS + [WORLD] + STOPS)
    rng = random.Random(0x1994_0C98)
    total = int(os.environ.get('N', 2000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        w, _, _ = setup(seed)
        a.mu.mem_write(a.lin(a.ds, 0), a.base)
        for off, data in w:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = relocated(a, a.seg_bytes(a.ds), FAR)
        try:
            a._setup([0xFFF0], {})
            a._run(0, 0x0C98, STUB_SEG, 0, count=5_000_000)
            after = relocated(a, a.seg_bytes(a.ds), FAR)
            res = f'{fnv(changes(before, after)):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'newgamenext | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
