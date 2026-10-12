"""Emit test vectors for src/travel.rs (1194:2E69, travel time) from the
original code.

Usage: gen_travel.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer and the game's heap. Start and end are
real locations from the game's CITYDATA.nn (slot 64 or 65): province,
x and y from the province's location entries. Weather and time are
random.

Lines: travel | slot dprov dx dy hprov hx hy minutes weather_hex | days digest
       (FNV-1a of the changed DGROUP bytes), or "fault".
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_2E69)
out = open(OUT, 'w', buffering=1)


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


def locations(slot):
    data = open(os.path.join(GAME, f'CITYDATA.{slot}'), 'rb').read()
    locs = []
    for p in range(9):
        block = data[p * 0x4CC:(p + 1) * 0x4CC]
        for k in range(48):
            e = 0x1C + 25 * k
            x, y = struct.unpack_from('<HH', block, e + 0x14)
            if block[e]:
                locs.append((p, x, y))
    return locs


LOCS = {s: locations(s) for s in (64, 65)}
a = DosArena(EXE, GAME)
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
r = a.call_near(0, 0x51F2, eax=0x1000)
a.w16(0x9030, r['dx'])
BASE = a.snapshot()
for _ in range(300):
    a.restore(BASE)
    slot = rng.choice([64, 65])
    dp, dx, dy = rng.choice(LOCS[slot])
    hp, hx, hy = rng.choice(LOCS[slot]) if rng.random() < 0.9 else (dp, dx, dy)
    minutes = rng.choice([rng.getrandbits(32), rng.randrange(360 * 1440 * 10)])
    weather = bytes(rng.choice([0, 0, rng.randint(0, 7), rng.getrandbits(8)]) for _ in range(36))
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), bytes([slot]))
    a.mu.mem_write(a.lin(a.ds, 0x193D), bytes([dp]))
    a.w16(0x193E, dx)
    a.w16(0x1940, dy)
    a.mu.mem_write(a.lin(a.ds, 0xA46), bytes([hp]))
    a.w16(0xA48, hx)
    a.w16(0xA4A, hy)
    a.w32(0x5CC, minutes)
    a.mu.mem_write(a.lin(a.ds, 0xA5A), weather)
    before = a.seg_bytes(a.ds)
    try:
        days = a.call_far(0x1194, 0x2E69)
        if a.r16(0xA90A) == a.ds:
            a.w16(0xA90A, a.ds - LOAD_SEG)
        after = a.seg_bytes(a.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        res = f'{days:x} {fnv(changed):x}'
    except (Fault, AssertionError):
        res = 'fault'
    out.write(f'travel | {slot} {dp:x} {dx:x} {dy:x} {hp:x} {hx:x} {hy:x} {minutes:x} {weather.hex()} | {res}\n')
out.close()
