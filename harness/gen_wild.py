"""Emit test vectors for src/wild.rs (217B:0004, the wilderness grid)
from the original code.

Usage: gen_wild.py ACD_unpacked.EXE GAME_DIR OUT

Lines: wild | seed template | digests   FNV-1a of the buffer segment
       (64 KiB) and of the changed DGROUP bytes; or "fault".
       block | slot block | digests     217B:0189 under the DOS layer and
       the game's heap: the floor, wall and ceiling planes (32 KiB), the
       buffer segment and the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from dos import DosArena

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_217B)
out = open(OUT, 'w', buffering=1)
BUF = 0x7000


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


a = Arena(EXE)
a.w16(0x9030, BUF)
BASE = bytes(a.mu.mem_read(0, 0x110000))
templates = [b'city1.mif', b'cityw1.mif', b'cityw2.mif', b'cityw3.mif', b'town2.mif', b'townw1.mif',
             b'village3.mif', b'villagw1.mif', b'villagw4.mif', b'imperial.mif', b'cw9.mif', b'twz']
for _ in range(300):
    a.mu.mem_write(0, BASE)
    seed = rng.choice([rng.getrandbits(32), struct.unpack('<I', bytes(rng.randint(0x41, 0x7A) for _ in range(4)))[0], 0])
    template = rng.choice(templates)
    a.w32(0xA22, seed)
    a.mu.mem_write(a.lin(a.ds, 0xA36), template + b'\0')
    a.w32(0x91F6, rng.getrandbits(32))
    a.w16(0x91F0, 0)
    before = a.seg_bytes(a.ds)
    try:
        a.call_far(0x217B, 0x0004)
        after = a.seg_bytes(a.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        res = f'{fnv(a.seg_bytes(BUF)):x} {fnv(changed):x}'
    except Fault:
        res = 'fault'
    out.write(f'wild | {seed:x} {template.decode()} | {res}\n')

d = DosArena(EXE, GAME)
d.init_heap()
d.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
for var, paras in ((0x9030, 0x1000), (0xCCD0, 0x800), (0xD806, 0x800), (0xD748, 0x800)):
    r = d.call_near(0, 0x51F2, eax=paras)
    d.w16(var, r['dx'])
r = d.call_near(0, 0x51EC, eax=0x8000)
d.w16(0xBA54, r['ax'])
d.w16(0xBA56, r['dx'])
DBASE = d.snapshot()
for _ in range(200):
    d.restore(DBASE)
    slot = rng.randrange(4)
    block = rng.choice([rng.randint(5, 70)] * 6 + [rng.randint(1, 4), 0, rng.randint(71, 255)])
    before = d.seg_bytes(d.ds)
    try:
        d.call_near(0x217B, 0x0189, eax=block << 8 | slot)
        if d.r16(0xA90A) == d.ds:
            d.w16(0xA90A, d.ds - LOAD_SEG)
        after = d.seg_bytes(d.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        parts = [fnv(d.mu.mem_read(d.lin(d.r16(v), 0), 0x8000)) for v in (0xCCD0, 0xD806, 0xD748)]
        parts += [fnv(d.seg_bytes(d.r16(0x9030))), fnv(changed)]
        res = ' '.join(f'{p:x}' for p in parts)
    except (Fault, AssertionError):
        res = 'fault'
    out.write(f'block | {slot:x} {block:x} | {res}\n')
out.close()
