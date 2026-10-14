"""Emit test vectors for src/names.rs (NPC names, 1194:080B) from the
original code.

Usage: gen_names.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer (harness/dos.py) with the user's
NAMECHNK.DAT. The name is written to OUT_SEG:off. Vectors hold only the
inputs and FNV-1a digests (of the 64 bytes at the output and of the
changed DGROUP bytes), not the names.

Lines:
  name   | kind sex seed off | digests, or "fault"
  expand | seed race class level sex roll rng rec | digests   a whole NPC
         (1194:351D) with its name, seed record at SSEG:0, record at
         CSEG:rec, the creature segment's RNG seed `rng` (as in
         gen_creature.py); digests of the record, the RNG state there and
         the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG
from emu import changes

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_080B)
out = open(OUT, 'w', buffering=1)
OUT_SEG = 0x7000


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


a = DosArena(EXE, GAME)
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
BASE = a.snapshot()

cases = [(k, s) for k in range(32) for s in (0, 1)] * 20
cases += [(rng.getrandbits(8), rng.getrandbits(8)) for _ in range(150)]
for kind, sex in cases:
    a.restore(BASE)
    seed = rng.choice([rng.getrandbits(32), rng.getrandbits(32) | 1])
    off = rng.choice([0, 0x100, rng.getrandbits(16) & 0xFFC0])
    a.w32(0x91F6, rng.getrandbits(32))
    a.w16(0x91F0, 0)
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0x1194, 0x080B, eax=kind | sex << 8, ebx=seed, es=OUT_SEG, edi=off)
        if a.r16(0xA90A) == a.ds:
            a.w16(0xA90A, a.ds - LOAD_SEG)
        after = a.seg_bytes(a.ds)
        changed = changes(before, after)
        name = bytes(a.mu.mem_read(a.lin(OUT_SEG, 0), 0x10000))
        name = (name + name)[off:off + 64]
        res = f'{fnv(name):x} {fnv(changed):x}'
    except Fault:
        res = 'fault'
    out.write(f'name | {kind:x} {sex:x} {seed:x} {off:x} | {res}\n')

# Image-relative, as in gen_creature.py; the emulator relocates them.
CSEG, SSEG = 0x4C00, 0x5100
RC, RS = LOAD_SEG + CSEG, LOAD_SEG + SSEG
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
for _ in range(300):
    a.restore(BASE)
    seed = rng.getrandbits(32) | 1
    race = rng.randint(0, 7) | (0x80 if rng.random() < 0.2 else 0)
    klass = rng.choice(CLASS_IDS)
    level = rng.randint(0, 30)
    sex = rng.choice([0, 1])
    crng = rng.getrandbits(32) | 1
    rec = rng.randrange(8) * 0x41E
    roll = rng.getrandbits(8)
    a.mu.mem_write(a.lin(RS, 0), struct.pack('<IBBBBB', seed, race, klass, level, sex, roll))
    a.mu.mem_write(a.lin(RC, 0x91F6), struct.pack('<I', crng))
    a.mu.mem_write(a.lin(RC, 0x91F0), b'\0\0')
    a.w32(0x91F6, rng.getrandbits(32) | 1)
    a.w16(0x91F0, 0)
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0x1194, 0x351D, ds=RS, esi=0, es=RC, edi=rec)
        if a.r16(0xA90A) == a.ds:
            a.w16(0xA90A, a.ds - LOAD_SEG)
        after = a.seg_bytes(a.ds)
        changed = changes(before, after)
        record = bytes(a.mu.mem_read(a.lin(RC, rec), 0x41E))
        state = bytes(a.mu.mem_read(a.lin(RC, 0x91EE), 0xE))
        res = f'{fnv(record):x} {fnv(state):x} {fnv(changed):x}'
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'expand | {seed:x} {race:x} {klass:x} {level:x} {sex:x} {roll:x} {crng:x} {rec:x} | {res}\n')
out.close()
