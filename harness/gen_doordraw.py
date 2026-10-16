"""Emit test vectors for how the renderer draws a door ajar (src/doordraw.rs):
the texture an opening door is drawn with (0000:BAF3, called from ABB3
when CB38 >= 1 and CCC4 isn't 0) and a swinging door's far edge turned
about its hinge (0000:D344, through the rotation 0000:93BC).

Usage: gen_doordraw.py ACD_unpacked.EXE OUT

The textures' segment ([9030], TSEG) holds the same 64K of bytes for
every case (the LCG stream from seed TEXSEED); the output ([BA54] far,
OSEG:0) starts each case as EEh.

  tex | seed | digest of OSEG's first 1100h bytes
  swing | seed | D7B0 D7B4 D7B6 D7BA
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import mine
from gen_attack import fnv, stream

TSEG = 0x7000
OSEG = 0x8000
TEXSEED = 0xD00D_0B0B
SPAN = 0x1100


def tex_setup(seed):
    """(kind CB38, open CCC4, texture offset [si+10]). Mirrored in
    tests/doordraw.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    kind = byte() % 4
    opening = 1 + byte() % 7
    slot = byte() % 15
    return kind, opening, slot * 0x1000


def swing_setup(seed):
    """The words written: D7AA, D7AE (the hinge), D7B0, D7B4 (the far
    end), CCC4, and CB38's byte. Mirrored in tests/doordraw.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    hinge = (word(), word())
    if byte() & 1:
        # A face as the renderer sets it: the square's corner plus 0 or
        # 80h each way.
        x, y = (byte() & 0x7F) << 7, byte() << 7
        corners = [(0, 0), (0x80, 0), (0x80, 0x80), (0, 0x80)]
        k = byte() % 4
        hinge = (x + corners[k][0], y + corners[k][1])
        far = (x + corners[(k + 1) % 4][0], y + corners[(k + 1) % 4][1])
    else:
        far = (word(), word())
    opening = byte() % 8 if byte() & 1 else word()
    kind = 0 if byte() & 1 else byte()
    return [h & 0xFFFF for h in hinge] + [f & 0xFFFF for f in far] + [opening, kind]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    texs = stream(TEXSEED)
    a.mu.mem_write(a.lin(TSEG, 0), bytes(next(texs) for _ in range(0x10000)))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_BAF3)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        if n % 2 == 0:
            kind, opening, off = tex_setup(seed)
            a.mu.mem_write(a.lin(OSEG, 0), b'\xee' * SPAN)
            a.mu.mem_write(a.lin(a.ds, 0xCB38), bytes([kind]))
            a.w16(0xCCC4, opening)
            a.w16(0x9030, TSEG)
            a.w16(0xBA54, 0)
            a.w16(0xBA56, OSEG)
            # The record SI points at: its texture's offset at +10h.
            a.w16(0x7F00 + 0x10, off)
            try:
                a.call_near(0, 0xBAF3, esi=0x7F00)
                res = f'{fnv(bytes(a.mu.mem_read(a.lin(OSEG, 0), SPAN))):x}'
            except Fault:
                res = 'fault'
            except AssertionError:
                res = 'hang'
            out.write(f'tex | {seed:x} | {res}\n')
        else:
            v = swing_setup(seed)
            for at, w in zip((0xD7AA, 0xD7AE, 0xD7B0, 0xD7B4, 0xCCC4), v):
                a.w16(at, w)
            a.mu.mem_write(a.lin(a.ds, 0xCB38), bytes([v[5]]))
            try:
                a.call_near(0, 0xD344)
                res = ' '.join(f'{a.r16(at):x}' for at in (0xD7B0, 0xD7B4, 0xD7B6, 0xD7BA))
            except Fault:
                res = 'fault'
            except AssertionError:
                res = 'hang'
            out.write(f'swing | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
