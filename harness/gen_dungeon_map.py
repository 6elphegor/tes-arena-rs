"""Emit test vectors for whole dungeon levels (src/dungeon.rs:
dungeon_create, 0000:28BC..2938, and dungeon_build_level, 0000:299F)
from the original code, with its file I/O.

Usage: gen_dungeon_map.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer with the game's heap (harness/dos.py). Each
case allocates the 64 KB buffer (DGROUP 9030) and the chunk block
(BA54) from the heap, as the game does, creates a dungeon and builds one
of its levels. Vectors hold only settings and FNV-1a digests: of the
changed DGROUP bytes (offset and value), the floor and map planes (32 KB
each), the buffer (64 KB) and the chunk block.

Lines: dungeon | ax bx levels level seed c398 c39a | digests
The result is "fault" for a CPU fault and "hang" when the instruction
budget runs out.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_299F)
out = open(OUT, 'w', buffering=1)
CHUNK_BLOCK = 0x3E80


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


a = DosArena(EXE, GAME)
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
BASE = a.snapshot()


def digests(before):
    if a.r16(0xA90A) == a.ds:
        a.w16(0xA90A, a.ds - LOAD_SEG)
    after = a.seg_bytes(a.ds)
    changed = bytearray()
    for i in range(0x10000):
        if after[i] != before[i]:
            changed += struct.pack('<HB', i, after[i])
    parts = [fnv(changed)]
    for var, n in ((0xCCD0, 0x8000), (0xD806, 0x8000), (0x9030, 0x10000), (0xBA56, CHUNK_BLOCK)):
        s = a.r16(var)
        parts.append(fnv(a.seg_bytes(s)[:n]) if s else 0)
    return ' '.join(f'{p:x}' for p in parts)


for _ in range(150):
    a.restore(BASE)
    ax = rng.choice([0, 0, 1, 1, 2, 3])
    bx = rng.choice([0, 0, 1, 1, 2, 3])
    levels = rng.choice([1, 2, 3, 4, rng.randint(1, 8)])
    level = rng.choice([rng.randint(0, levels - 1)] * 4 + [levels, rng.getrandbits(8)])
    seed = rng.getrandbits(32)
    c398, c39a = rng.getrandbits(16), rng.getrandbits(16)
    r = a.call_near(0, 0x51F2, eax=0x1000)
    a.w16(0x9030, r['dx'])
    r = a.call_near(0, 0x51EC, eax=CHUNK_BLOCK)
    a.w16(0xBA54, r['ax'])
    a.w16(0xBA56, r['dx'])
    a.w32(0x91F6, seed)
    a.w16(0x91F0, 0)
    a.w16(0xC398, c398)
    a.w16(0xC39A, c39a)
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0, 0x28BC, eax=ax, ebx=bx, ecx=levels)
        a.call_near(0, 0x299F, ecx=level)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'dungeon | {ax:x} {bx:x} {levels:x} {level:x} {seed:x} {c398:x} {c39a:x} | {res}\n')
out.close()
