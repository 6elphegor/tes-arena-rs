"""Emit test vectors for the state the original sets as it starts
drawing the view (src/engine.rs `view_begin`, 0000:C923, with the walk
over the squares, 0000:C980, stubbed): the player's square and column
and row, the wall under the player, and the draw's resets.

Usage: gen_view.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/view.rs) the
player's position (C3D0, C3D4), the flags (C398), the variables the
start resets, and the wall plane around the player.

  view | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes
from emu import mine
from gen_attack import fnv, stream

WSEG = 0x6800


def setup(seed):
    """Writes. Mirrored in tests/view.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    x, y = word(), word()
    w.append((None, 0xC3D0, pk(x)))
    w.append((None, 0xC3D4, pk(y)))
    w.append((None, 0xC398, pk(word())))
    for at in (0xCB92, 0xCCCA, 0xCD14, 0xCCB2, 0xCCB4, 0xA854, 0xA856, 0xCCA4, 0xCCA6, 0xCB90, 0xCB6C):
        w.append((None, at, pk(word())))
    sq = ((y >> 7) << 8) + ((x >> 7) << 1)
    for d in (0, 2, 0x100, 0xFFFE):
        w.append((WSEG, (sq + d) & 0xFFFF, pk(word())))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0xD806, WSEG)
    # The walk over the squares draws; its state is the front end's.
    a.mu.mem_write(a.lin(LOAD_SEG, 0xC980), b'\xc3')
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_C923)
    total = int(os.environ.get('N', 2000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0, 0xC923)
            after = a.seg_bytes(a.ds)
            changed = changes(before, after)
            res = f'{fnv(changed):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'view | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
