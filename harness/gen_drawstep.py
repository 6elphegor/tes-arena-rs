"""Emit test vectors for an object's animation step as it is drawn
(src/projectile.rs `drawn_step`, 0000:C23A..C26C) from the original code.

Usage: gen_drawstep.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/drawstep.rs) the
first projectile slot (7DBC, 1Ch bytes): its flags (+F: random, with bits
2004h and 100h drawn on their own), frame (+D), frame count (+E), light
(+6: 0, one of the 32 lights at BB0C or any word) and the light count
(BC8C, around 20h), then runs 0000:C23A..C28F with BX at the slot.

  drawstep | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, changes, fnv, mine
from gen_attack import stream

SLOT = 0x7DBC


def setup(seed):
    """(offset, bytes) writes into DGROUP. Mirrored in tests/drawstep.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    rec = bytearray(byte() for _ in range(0x1C))
    flags = word() & ~0x2104
    if byte() < 0x40:
        flags |= (0x4, 0x2000, 0x2004)[byte() % 3]
    if byte() < 0x80:
        flags |= 0x100
    struct.pack_into('<H', rec, 0xF, flags)
    count = byte() % 0x10 | (0x80 if byte() & 1 else 0)
    rec[0xE] = count
    rec[0xD] = ((count & 0x7F) - 1 + byte() % 3 if byte() < 0xC0 else byte()) & 0xFF
    k = byte() % 4
    light = (0, 0xBB0C + 6 * (byte() % 32), word(), 0xBB0C)[k]
    struct.pack_into('<H', rec, 6, light)
    return [(SLOT, bytes(rec)), (0xBC8C, struct.pack('<H', byte() % 0x22))]


def main():
    EXE, OUT = sys.argv[1:3]
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_C23A)
    out = open(OUT, 'w', buffering=1)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.run_range(0, 0xC23A, 0xC28F, bx=SLOT)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'drawstep | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
