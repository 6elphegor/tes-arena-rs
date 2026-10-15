"""Emit test vectors for the interface bar's status (src/status.rs:
`time_text` 0000:1486, `carried_weight` 1194:4E17, `status_text`
2542:04B1..04F6, `pilfer_arm` 2542:0279).

Usage: gen_status.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/status.rs) the
clock (5CC: any dword, or one aimed at a whole hour, noon or midnight),
the player's 40 inventory slots (212h, 13h bytes each: their flags at
+0Eh, occupied (40h) mostly for the first few, and their weight word at
+1), the most the player carries (1DF), the place's name (A22, letters
and a NUL) and the colour 9288; then runs one op:

  0 0000:1486 with SI = 1144h (the time)
  1 1194:4E17 (the weight; AX)
  2 2542:04B1 up to its font calls (04F7)
  3 2542:0279

  status | seed | op | ax | digest

The digest is FNV-1a of the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, changes, fnv, mine
from gen_attack import stream


def setup(seed):
    """(writes, op). Mirrored in tests/status.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    op = byte() % 4
    k = byte() % 4
    if k == 0:
        clock = word() | word() << 16
    else:
        # A whole hour (noon, midnight among them) or a minute after one.
        clock = ((word() % 2000) * 1440 + (byte() % 24) * 60 + (0, 0, 1, 9, 10, 59)[byte() % 6])
    w.append((0x05CC, struct.pack('<I', clock & 0xFFFFFFFF)))
    full = byte() % 41
    for i in range(40):
        rec = bytearray(byte() for _ in range(0x13))
        f = rec[0xE] & ~0x40
        if i < full and byte() < 0xF0:
            f |= 0x40
        rec[0xE] = f
        if byte() < 0x40:
            rec[1] = byte()
            rec[2] = 0
        w.append((0x0212 + 0x13 * i, bytes(rec)))
    w.append((0x01DF, struct.pack('<H', word())))
    n = byte() % 16
    w.append((0x0A22, bytes(0x41 + byte() % 26 for _ in range(n)) + b'\0'))
    w.append((0x9288, bytes([byte()])))
    return w, op


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_04B1)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, op = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                ax = a.call_far(0x0000, 0x1486, si=0x1144)
            elif op == 1:
                ax = a.call_far(0x1194, 0x4E17)
            elif op == 2:
                ax = a.run_range(0x2542, 0x04B1, 0x04F7)['ax']
                ax = 0
            else:
                ax = a.call_near(0x2542, 0x0279)['ax']
                ax = 0
            if op == 0:
                ax = 0
            # The message's segment as the image has it (DGROUP 3BA0h),
            # not as loaded here.
            if a.r16(0x061C) == a.ds:
                a.w16(0x061C, 0x3BA0)
            res = f'{ax:x} | {fnv(changes(before, a.seg_bytes(a.ds))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'status | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
