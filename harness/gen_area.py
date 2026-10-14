"""Emit test vectors for two pieces of entering an area (src/frame.rs
`ems_reset`, 0000:5D74, and `weather_drops`, 0000:E322) from the
original code.

Usage: gen_area.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/area.rs) the EMS
switch and bank counts, the bank table, the weather, the view's
rectangle and the random seed; then runs 0000:5D74 and 0000:E322.

  area | seed | result

Results: "fault" / "hang", or an FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import changes
from emu import mine
from gen_attack import fnv, stream


def setup(seed):
    """(offset, bytes) DGROUP writes. Mirrored in tests/area.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x8FA6, bytes([1 if byte() < 0xC0 else 0])))
    n = byte() % 0x40
    w.append((0x9032, pk(n)))
    w.append((0x9034, pk(byte() % (n + 1) if byte() < 0xF0 else n)))
    w.append((0x8FAA, bytes(byte() for _ in range(0x80))))
    w.append((0xBB09, bytes([0x82 if byte() < 0xA0 else byte()])))
    x0, y0 = byte() % 0x40, byte() % 0x40
    w.append((0xA914, pk(x0)))
    w.append((0xA916, pk(y0)))
    w.append((0xA918, pk(x0 + 1 + byte() if byte() < 0xF0 else x0)))
    w.append((0xA91A, pk(y0 + 0x15 + byte() if byte() < 0xF0 else y0 + 0x14)))
    w.append((0xE05E, bytes(byte() for _ in range(0x40))))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0x91F0, 0)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_A4ED)
    total = int(os.environ.get('N', 2000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0, 0x5D74)
            a.call_near(0, 0xE322)
            after = a.seg_bytes(a.ds)
            changed = changes(before, after)
            res = f'{fnv(changed):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'area | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
