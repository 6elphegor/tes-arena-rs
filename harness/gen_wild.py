"""Emit test vectors for src/wild.rs (217B:0004, the wilderness grid)
from the original code.

Usage: gen_wild.py ACD_unpacked.EXE OUT

Lines: wild | seed template | digests   FNV-1a of the buffer segment
       (64 KiB) and of the changed DGROUP bytes; or "fault".
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault

EXE, OUT = sys.argv[1:3]
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
out.close()
