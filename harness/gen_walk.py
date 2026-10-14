"""Emit test vectors for the doors stepping as the original walks the
squares in view (src/engine.rs `view_walk_doors`): the world loop's call
of the view (0000:A362..A388: C398 bit 20h by the height, then C923 with
its walk, drawing included) after `view_begin`'s state is set.

Usage: gen_walk.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/walk.rs) the map's
size (C357, C359), the player's position and height (C3D0..C3D4), the
reach (C394, 1..8), C398 and C39A, and the wall plane around the player:
empty squares, full walls, kind 0Dh walls, and doors (kind 0Bh, any
state and low byte). The door hook (CD46) is unset.

  walk | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the wall plane.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import mine
from gen_attack import fnv, stream

WSEG = 0x6800


def setup(seed):
    """Writes. Mirrored in tests/walk.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    cols, rows = 8 + byte() % 121, 8 + byte() % 121
    w.append((None, 0xC357, pk(cols)))
    w.append((None, 0xC359, pk(rows)))
    # Mostly in the map, sometimes off its edge.
    x = (byte() % (cols + 4)) << 7 | byte() & 0x7F
    y = (byte() % (rows + 4)) << 7 | byte() & 0x7F
    w.append((None, 0xC3D0, pk(x)))
    w.append((None, 0xC3D4, pk(y)))
    h = word()
    w.append((None, 0xC3D2, pk(h & 0x7F if byte() & 3 else h)))
    reach = 1 + byte() % 8
    w.append((None, 0xC394, pk(reach)))
    w.append((None, 0xC398, pk(word() & ~0x24)))
    w.append((None, 0xC39A, pk(word() & ~0x204 | (0x4 if byte() % 8 == 0 else 0))))
    w.append((None, 0xCD46, pk(0)))
    w.append((None, 0xCD48, pk(0)))
    col, row = x >> 7, y >> 7
    for dr in range(-reach - 1, reach + 2):
        for dc in range(-reach - 1, reach + 2):
            k = byte() % 10
            if k < 4:
                v = 0
            elif k < 6:
                t = 1 + byte() % 0x3F
                v = t << 8 | t
            elif k < 7:
                t = byte()
                v = 0xD000 | (t & 0xF) << 8 | t
            else:
                v = 0xB000 | word() & 0x0FFF
            w.append((WSEG, ((row + dr) << 8) + ((col + dc) << 1), pk(v)))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0xD806, WSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_C980)
    total = int(os.environ.get('N', 1500))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        try:
            a.run_range(0, 0xA362, 0xA388)
            res = f'{fnv(a.seg_bytes(WSEG)):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'walk | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
