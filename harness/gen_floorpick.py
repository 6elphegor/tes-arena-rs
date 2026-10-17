"""Emit test vectors for the floor under the cursor (src/click.rs,
`Machine::floor_pick`): 2826:0B52, which projects the cursor (8F92, 8F94)
onto the floor through 0000:DB43 and the rotation 0000:93BC.

Usage: gen_floorpick.py ACD_unpacked.EXE OUT

  pick | seed | SI (FFFF: the carry) or fault
"""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_SI, UC_X86_REG_EFLAGS

WORDS = (0x8F92, 0x8F94, 0xC3D8, 0xA91A, 0xC3D2, 0xCCF6, 0xC3D6, 0xCCF4, 0xC3DA, 0xC3D0, 0xC3D4, 0xC357, 0xC359)


def setup(seed):
    """The words written, in WORDS' order. Mirrored in tests/floorpick.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    if byte() % 8 == 0:
        return [word() for _ in WORDS]
    # As play has them: the cursor in the view, the horizon about
    # mid-view, the eye's height, the projection's scales, the heading,
    # the eye on a map of up to 128 squares each way.
    horizon = 0x40 + byte() % 0x30
    cols, rows = 0x20 + byte() % 0x60, 0x20 + byte() % 0x60
    v = [byte() % 0x140, horizon - 8 + byte() % 0x50, horizon, 0x92 - horizon + byte() % 0x20]
    v += [0x32 if byte() & 1 else byte() << 1, 0x80 + byte() % 0x80, 0xA0, 0x80 + byte() % 0x80]
    v += [word() & 0x1FF]
    x = (byte() % cols) << 7
    x |= byte() & 0x7F
    y = (byte() % rows) << 7
    y |= byte() & 0x7F
    v += [x, y, cols, rows]
    return [x & 0xFFFF for x in v]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0B52)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for at, w in zip(WORDS, setup(seed)):
            a.w16(at, w)
        try:
            a.call_near(0x2826, 0x0B52, esi=0xFFFF)
            si = a.mu.reg_read(UC_X86_REG_SI)
            cf = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
            res = 'ffff' if cf else f'{si:x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'pick | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
