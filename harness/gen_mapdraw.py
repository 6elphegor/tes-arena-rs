"""Emit test vectors for the automap screen's squares (src/mapdraw.rs:
`automap_squares`, 0000:2F20..2F47 with 2F6D, 303C, 3161, 3825 and
23F1:01E1, 0219, 02D1).

Usage: gen_mapdraw.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/mapdraw.rs) the
place (C398, C39A: a city, a dungeon, the wilderness), the level's size
(up to 90h wide: a column doubled past FFh wraps in the planes),
the view's top left (A880, A882: near the level, past it, near 7FFFh or
any word), the player's square and heading, the exit's, the stairs' and
the trigger walls' numbers (A865, A866, C917..C926, C943: small, as the
walls' are), the solid floor (CCCE); then from an LCG the walls plane
(segment 6000h at D806), the floors plane (7000h at CCD0) and the seen
bytes (8000h at 9030) over the level, and the page (A000h at A900) with
a tile of colours near the blend's range. It runs 2F20..2F47.

  mapdraw | seed | <DGROUP digest> <page digest>, or hang

(A left of 8000h hangs: 2F6D counts the columns down to it by a signed
`jge`, which never fails at the most negative word.)

The digests are FNV-1a of the changed DGROUP bytes and of the page's
64K.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, changes, fnv, mine
from gen_attack import stream

WALLS, FLOORS, SEEN, PAGE = 0x6000, 0x7000, 0x8000, 0xA000
TILE = 997


def lcg(x):
    x = (x * 1103515245 + 12345) & 0xFFFFFFFF
    return x, x >> 16


def setup(seed):
    """(DGROUP writes, {segment: {offset: bytes}}, page). Mirrored in
    tests/mapdraw.rs (a tuple's items before its index)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    mode = byte() % 4
    c398, c39a = word(), word()
    if mode == 0:
        c398 |= 4
        c39a |= 0x200
    elif mode == 1:
        c398 &= ~4
    elif mode == 2:
        c398 |= 4
        c39a &= ~0x200
    w = 1 + byte() % 0x90
    d = 1 + byte() % 0x40
    lefts = (word(), 0x7FE0 + byte() % 0x40, byte() % 0x60 - 0x30, byte() % 0xA0 - 0x30)
    left = lefts[byte() % 4] & 0xFFFF
    tops = (word(), byte() % 0x50 - 0x20, byte() % 0x50 - 0x20)
    top = tops[byte() % 3] & 0xFFFF
    px = (left + byte() % 0x3C) & 0xFFFF
    py = (top + byte() % 0x2A) & 0xFFFF
    heading = word()
    exit_, stairs = byte() % 8, byte() % 8
    triggers = bytes(byte() % 12 for _ in range(16))
    c943 = byte() % 8
    solid = 0x100 + (byte() % 4) * 0x100
    x = word() << 16 | word()
    writes = [(0xC398, pk(c398)), (0xC39A, pk(c39a)), (0xC357, pk(w)), (0xC359, pk(d)),
              (0xA880, pk(left)), (0xA882, pk(top)), (0xCCA4, pk(px)), (0xCCA6, pk(py)),
              (0xC3DA, pk(heading)), (0xA865, bytes([exit_])), (0xA866, bytes([stairs])),
              (0xC917, triggers), (0xC943, bytes([c943])), (0xCCCE, pk(solid)),
              (0xD806, pk(WALLS)), (0xCCD0, pk(FLOORS)), (0x9030, pk(SEEN)), (0xA900, pk(PAGE))]
    walls, floors, seen = bytearray(0x10000), bytearray(0x10000), bytearray(0x10000)
    for r in range(d):
        for c in range(w):
            i = r << 8 | (c * 2 & 0xFF)
            x, v = lcg(x)
            x, u = lcg(x)
            t = v & 7
            if t == 0:
                wall = 0
            elif t == 1:
                b = u & 7 | (u >> 8) & 0x40
                wall = b | b << 8
            elif t == 2:
                wall = u & 0x7F07
            elif t in (3, 4):
                wall = 0x8000 | ((v >> 3) & 7) << 12 | (u & 0x0F07)
            elif t == 5:
                wall = u & 0x0107
            elif t == 6:
                wall = u
            else:
                wall = 0x8000 | (u & 0x0F07)
            x, f = lcg(x)
            floor = (solid - 1, solid, solid + 0x100, SEEN + 0x100, solid + 0x200, f)[f % 6]
            x, g = lcg(x)
            sn = (0, 7, 0xE, 0x15, g >> 8)[g % 5]
            struct.pack_into('<H', walls, i, wall)
            struct.pack_into('<H', floors, i, floor & 0xFFFF)
            seen[i >> 1] = sn
    tile = bytearray()
    for _ in range(TILE):
        x, v = lcg(x)
        tile.append((v >> 8) & 0xFF if v & 1 else 0x30 + (v >> 8) % 0x50)
    page = (bytes(tile) * (0x10000 // TILE + 1))[:0x10000]
    return writes, {WALLS: walls, FLOORS: floors, SEEN: seen}, page


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_2F6D)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, segs, page = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        for seg, data in segs.items():
            a.mu.mem_write(a.lin(seg, 0), bytes(data))
        a.mu.mem_write(a.lin(PAGE, 0), page)
        before = a.seg_bytes(a.ds)
        try:
            a.run_range(0, 0x2F20, 0x2F47)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x} {fnv(a.seg_bytes(PAGE)):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'mapdraw | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
