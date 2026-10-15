"""Emit test vectors for the automap screen's view (src/automap.rs:
`automap_view_button` 0000:37ED/37F7/3801/380B/3815, `automap_centre`
3796, `automap_origin` 377D, `automap_view_open` 2D1F..2D2E).

Usage: gen_mapview.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/mapview.rs) the
level's size (C357, C359: mostly 60 x 42 or more), the player's place
(C3D0, C3D4) and the screen's centre (126C, 126E: any word, or one
aimed at an edge of the clamp, give or take 1), then runs one op:

  0..4  a button's handler (near)
  5     3796
  6     377D
  7     2D1F..2D31

  mapview | seed | op | digest

The digest is FNV-1a of the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, changes, fnv, mine
from gen_attack import stream

BUTTONS = (0x37ED, 0x37F7, 0x3801, 0x380B, 0x3815)


def setup(seed):
    """(writes, op). Mirrored in tests/mapview.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() % 8
    w = 0x3C + byte() % 0x45 if byte() < 0xE0 else byte() % 0x3C
    d = 0x2A + byte() % 0x57 if byte() < 0xE0 else byte() % 0x2A
    xs = (word(), 0xF00 + byte() % 3 - 1, ((w << 7) - 0xE80) + byte() % 3 - 1, word() % (w << 7 | 1))
    ys = (word(), 0xA7F + byte() % 3 - 1, ((d << 7) - 0xA00) + byte() % 3 - 1, word() % (d << 7 | 1))
    x = xs[byte() % 4]
    y = ys[byte() % 4]
    px, py = word(), word()
    return [(0xC357, pk(w)), (0xC359, pk(d)), (0x126C, pk(x)), (0x126E, pk(y)), (0xC3D0, pk(px)), (0xC3D4, pk(py))], op


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_2CFE)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, op = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            if op < 5:
                a.call_near(0, BUTTONS[op])
            elif op == 5:
                a.call_near(0, 0x3796)
            elif op == 6:
                a.call_near(0, 0x377D)
            else:
                a.run_range(0, 0x2D1F, 0x2D31)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'mapview | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
