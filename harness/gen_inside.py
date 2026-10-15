"""Emit test vectors for the hook CD42's part indoors (src/hooks.rs
`area_entered_inside`, 0000:4767..47D6 of 0000:468B: a noble's house
furnished (0000:5160), a house's flats marked (513A), the level's lights,
the listed flats marked, the main quest's item placed, a side quest's
level, the piles, a quest's flat cleared, the floor's marks, the palace's
screen) from the original code.

Usage: gen_inside.py ACD_unpacked.EXE OUT

A case fills from a seed harness/gen_hooks.py's `setup_level` (the level
op's state), then from a second stream (seed ^ 5EED1234h; `extras`,
mirrored with it in tests/inside.rs) 5CA (bit 4000h now and then), the
area 5E6 (a building or a dungeon), the building's kind D48 (8, 1 or
any), A52, some of the listed squares at C6E5 made FFFFh, and now and
then the palace (A47 3, A4C's low bits 0, A46 4). Stubbed, counting their
calls as for the level op: 292B:09AC (FFE0), 1194:9FAC (FFE2), 2433:0000
(FFE4), 0000:368E (FFE6).

  inside | seed | result

Results: "fault" / "hang" (past 400M instructions), or the FNV-1a digests of the changed DGROUP
bytes and of the wall, floor and creature segments.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, mine
from gen_attack import fnv, stream
from gen_frame import counter
from gen_hooks import WSEG, FSEG, CSEG, setup_level

pk = lambda v: struct.pack('<H', v & 0xFFFF)
pb = lambda v: bytes([v & 0xFF])


def extras(seed):
    """Writes after `setup_level`'s. Mirrored in tests/inside.rs."""
    s = stream(seed ^ 0x5EED1234)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    w.append((None, 0x05CA, pk(word() & ~0x4000 | (0x4000 if byte() < 0x20 else 0))))
    k = byte() % 5
    w.append((None, 0x05E6, pb((4, 4, 3, 3, byte())[k])))
    k = byte() % 4
    w.append((None, 0x0D48, pb((8, 8, 1, byte())[k])))
    w.append((None, 0x0A52, pb(byte())))
    for i in range(8):
        if byte() < 0x30:
            w.append((None, 0xC6E5 + 2 * i, pk(0xFFFF)))
    if byte() < 0x20:
        w.append((None, 0x0A47, pb(3)))
        w.append((None, 0x0A4C, pb(byte() & 0xE0)))
        w.append((None, 0x0A46, pb(4)))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, at in [(0x292B, 0x09AC, 0xFFE0), (0x1194, 0x9FAC, 0xFFE2), (0x2433, 0x0000, 0xFFE4), (0, 0x368E, 0xFFE6)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(at, True))
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_4767)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, FSEG, CSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup_level(seed) + extras(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before = a.seg_bytes(a.ds)
        try:
            # 400M instructions: 2294:119C's scan of a level 0 wide runs
            # 65536 words a row, past run_range's usual 50M.
            a._setup([], {})
            a._run(0, 0x4767, LOAD_SEG, 0x47D6, count=400_000_000)
            after = a.seg_bytes(a.ds)
            segs = ''.join(f' {fnv(a.seg_bytes(sg)):x}' for sg in (WSEG, FSEG, CSEG))
            res = f'{fnv(changes(before, after)):x}{segs}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'inside | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
