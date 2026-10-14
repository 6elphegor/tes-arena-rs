"""Emit test vectors for src/log.rs (the calendar 0000:1508 and log
entries 27E3:001A..005F) from the original code.

Usage: gen_log.py ACD_unpacked.EXE OUT

Lines:
  date  | minutes dest | digests      the date text (to its NUL) and the
                                      changed DGROUP bytes
  entry | minutes text_hex | ax digest  AX (the length) and the changed
                                      DGROUP bytes
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena
from emu import changes

EXE, OUT = sys.argv[1:3]
rng = random.Random(0x1994_1508)
out = open(OUT, 'w', buffering=1)
YEAR, DAY = 360 * 1440, 1440


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


a = Arena(EXE)
BASE = bytes(a.mu.mem_read(0, 0x110000))


def changed(before):
    after = a.seg_bytes(a.ds)
    return fnv(changes(before, after))


minutes = [0, 1, DAY - 1, DAY, 30 * DAY - 1, 30 * DAY, 11 * 30 * DAY, YEAR - 1, YEAR, 0xFFFFFFFF]
minutes += [d * DAY + rng.randrange(DAY) for d in range(0, 360, 7)]
minutes += [rng.getrandbits(32) for _ in range(200)] + [rng.randrange(10 * YEAR) for _ in range(200)]
for m in minutes:
    a.mu.mem_write(0, BASE)
    dest = rng.choice([0xA000, 0x9A6F, rng.randrange(0x9000, 0xF000)])
    before = a.seg_bytes(a.ds)
    a.call_far(0, 0x1508, eax=m, esi=dest)
    text = bytes(a.mu.mem_read(a.lin(a.ds, dest), 80)).split(b'\0')[0]
    out.write(f'date | {m:x} {dest:x} | {fnv(text):x} {changed(before):x}\n')

for _ in range(200):
    a.mu.mem_write(0, BASE)
    m = rng.choice([rng.getrandbits(32), rng.randrange(10 * YEAR)])
    text = bytes(rng.randint(0x20, 0x7E) for _ in range(rng.choice([0, 1, rng.randint(1, 200)])))
    a.w32(0x5CC, m)
    a.mu.mem_write(a.lin(a.ds, 0xA800), text + b'\0')
    before = a.seg_bytes(a.ds)
    r = a.run_range(0x27E3, 0x001A, 0x0060, esi=0xA800)
    out.write(f'entry | {m:x} {text.hex()} | {r["ax"]:x} {changed(before):x}\n')
out.close()
