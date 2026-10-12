"""Emit test vectors for src/movement.rs from the original code.

Usage: gen_move.py ACD_unpacked.EXE OUT

Lines:
  cell | x y seed flags | result   0000:F697 on the position at DGROUP F000:
       the cell value, and FNV-1a of the mask (929E, 256 bytes), E3A0,
       E8C2 and the wall plane. `flags` are the state written first
       (key=value pairs, hex); the planes and the door table (CDA1, 0x200 bytes) come from `seed`.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault

EXE, OUT = sys.argv[1:3]
rng = random.Random(0x1994_F697)
out = open(OUT, 'w', buffering=1)
WALLS, FLOOR = 0x7000, 0x7800


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


def plane_words(seed):
    """0x4000 words from an LCG, mirrored in tests/movement.rs: wall
    types in proportion (empty, plain, equal bytes, 8/9/A/B/C/D)."""
    x, words = seed, []
    for _ in range(0x4000):
        x = (x * 1103515245 + 12345) & 0xffffffff
        r = x >> 8
        t = r % 10
        v = (r >> 4) & 0xFFFF
        if t < 3:
            words.append(0)
        elif t == 3:
            words.append((v & 0x7F) * 0x101)
        elif t == 4:
            words.append(v & 0x7FFF)
        else:
            words.append((0x8 + (t - 5) % 6) << 12 | (v & 0x0FFF))
    return words


def door_bytes(seed):
    """0x200 door-table bytes from the same LCG (mirrored in the test)."""
    x, b = seed ^ 0x33333333, bytearray()
    for _ in range(0x200):
        x = (x * 1103515245 + 12345) & 0xffffffff
        b.append(x >> 16 & 0xFF)
    return bytes(b)


a = Arena(EXE)
a.w16(0xD806, WALLS)
a.w16(0xCCD0, FLOOR)
BASE = bytes(a.mu.mem_read(0, 0x110000))
VARS = [(0xE3A1, 1), (0xC3D2, 2), (0xC39A, 2), (0xC398, 2), (0xCCCE, 2), (0xD80C, 2), (0xE8C0, 2), (0xC38E, 2)]
for _ in range(1500):
    a.mu.mem_write(0, BASE)
    seed = rng.getrandbits(32)
    a.mu.mem_write(WALLS * 16, struct.pack('<16384H', *plane_words(seed)))
    a.mu.mem_write(FLOOR * 16, struct.pack('<16384H', *plane_words(seed ^ 0x5A5A5A5A)))
    vals = {0xE3A1: rng.choice([0, 0, 1]), 0xC3D2: rng.choice([0, 0x20, 0x3B, 0x3C, 0x50, rng.getrandbits(8)]),
            0xC39A: rng.getrandbits(16) & 0x0602, 0xC398: rng.getrandbits(16) & 4,
            0xCCCE: rng.choice([0, 0x100, rng.getrandbits(16)]), 0xD80C: rng.choice([0x96, rng.getrandbits(9)]),
            0xE8C0: rng.choice([0x30, rng.getrandbits(7)]), 0xC38E: rng.getrandbits(4)}
    for (o, n) in VARS:
        a.mu.mem_write(a.lin(a.ds, o), vals[o].to_bytes(n, 'little'))
    a.mu.mem_write(a.lin(a.ds, 0xCDA1), door_bytes(seed))
    x, y = rng.randint(0x100, 0x7E00), rng.randint(0x100, 0x7E00)
    a.w16(0xF000, x)
    a.w16(0xF002, y)
    try:
        r = a.call_near(0, 0xF697, esi=0xF000)
        d = a.seg_bytes(a.ds)
        res = f'{r["ax"] & 0xFF:x} {fnv(d[0x929E:0x939E]):x} {d[0xE3A0]:x} {fnv(d[0xE8C2:0xE8C4]):x} {fnv(a.seg_bytes(WALLS)[:0x8000]):x}'
    except Fault:
        res = 'fault'
    flags = ','.join(f'{o:x}={vals[o]:x}' for o, _ in VARS)
    out.write(f'cell | {x:x} {y:x} {seed:x} {flags} | {res}\n')
out.close()
