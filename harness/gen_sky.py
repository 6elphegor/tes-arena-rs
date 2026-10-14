"""Emit test vectors for the sky's colours (src/sky.rs `sky_colours`,
2294:0428, and `sky_step`, 2294:0341).

Usage: gen_sky.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/sky.rs) C398 (bit 4
half the time), the weather BB09, the time of day CCFE (mostly 0..3FFh),
and what is recorded as loaded (3C85, 3C89, 3C8B: mostly one of the
values the routines store); then runs one of the two. The file loads
(0000:7530 for the .COL, 0000:7428 for the dither) count their calls at
DGROUP FFE0 and FFE4; the palette writes (0000:9B52, called with DS at
the .COL's segment) count outside DGROUP.

  sky | seed | op | result

Op: 0 `sky_colours`, 1 `sky_step`. Results: "fault" / "hang", or the
FNV-1a digest of the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from gen_attack import fnv, stream
from gen_frame import counter

COLSEG = 0x7000


def setup(seed):
    """(writes, op). Mirrored in tests/sky.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    op = byte() & 1
    w.append((0xC398, pk(word() & ~4 | (4 if byte() & 1 else 0))))
    w.append((0xBB09, bytes([0 if byte() < 0x80 else byte()])))
    w.append((0xCCFE, pk(word() & 0x3FF if byte() < 0xE0 else word())))
    w.append((0x3C85, pk((0x42A2, 0x42AD, 0xFFFF)[byte() % 3] if byte() < 0xC0 else word())))
    w.append((0x3C89, pk((0x428B, 0x4296, 0xFFFF)[byte() % 3] if byte() < 0xC0 else word())))
    w.append((0x3C8B, pk(((word() & 0xFF) * 3, 0, 0xFFFF)[byte() % 3] if byte() < 0xC0 else word())))
    return w, op


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0, 0x7530, counter(0xFFE0, True)), (0, 0x9B52, counter(0xFFE2, True)),
                           (0, 0x7428, counter(0xFFE4, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0x43C0, COLSEG)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0341)
    for n in range(int(os.environ.get('N', 3000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, op = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x2294, (0x0428, 0x0341)[op])
            after = a.seg_bytes(a.ds)
            res = f'{fnv(b"".join(struct.pack("<HB", i, after[i]) for i in range(0x10000) if after[i] != before[i])):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'sky | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
