"""Emit test vectors for a building's hours (src/weapon.rs
`building_closed`, 1194:4FB8) from the original code.

Usage: gen_hours.py ACD_unpacked.EXE OUT

A case is a building kind (AL), a date (some on holidays, whose building
byte is sometimes the kind) and a time of day; 1194:4FB8 runs.

  hours | kind minutes tod | result     result: carry and 137F, hex
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena
from unicorn.x86_const import UC_X86_REG_EFLAGS


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    days = []
    at = 0x33AD
    while a.r16(at) <= 0x158:
        days.append((a.r16(at), len(days)))
        at += 2
    rng = random.Random(0x1994_4FB8)
    for _ in range(2000):
        if rng.random() < 0.5:
            day, n = rng.choice(days)
            kind = a.mu.mem_read(a.lin(a.ds, 0x3353 + 6 * n), 1)[0] if rng.random() < 0.6 else rng.randrange(10)
        else:
            day, kind = rng.randrange(360), rng.randrange(10)
        minutes = rng.randrange(30) * 518400 + day * 1440 + rng.randrange(1440)
        tod = rng.choice([0xFF, 0x100, 0x2FF, 0x300, rng.randrange(0x400)])
        a.w32(0x05CC, minutes)
        a.w16(0xCCFE, tod)
        a.call_far(0x1194, 0x4FB8, eax=kind)
        cf = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
        r137f = a.mu.mem_read(a.lin(a.ds, 0x137F), 1)[0]
        out.write(f'hours | {kind:x} {minutes:x} {tod:x} | {cf:x} {r137f:x}\n')
    out.close()


if __name__ == '__main__':
    main()
