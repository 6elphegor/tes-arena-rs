"""Emit test vectors for the outdoor INF choice (src/engine.rs
`outdoor_inf`, 0000:0896).

Usage: gen_outinf.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/outinf.rs) C398
(bit 4, outdoors, half the time), C39A (bit 200h, the wilderness, half
the time), the climate DA8 (mostly 0, 4 or 8) and the weather BB09 (mostly one
the game sets). The
townspeople's graphics (0000:17F9) count their calls at DGROUP FFE0.

  outinf | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from gen_attack import fnv, stream
from gen_frame import counter


def setup(seed):
    """Writes. Mirrored in tests/outinf.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0xC398, pk(word() & ~4 | (4 if byte() & 1 else 0))))
    w.append((0xC39A, pk(word() & ~0x200 | (0x200 if byte() & 1 else 0))))
    w.append((0x0DA8, pk((0, 4, 8)[byte() % 3] if byte() < 0xE0 else word())))
    # The weathers the game sets (0FAA..0FBF), or any.
    w.append((0xBB09, bytes([(0, 1, 2, 0x81, 0x82)[byte() % 5] if byte() < 0xC0 else byte()])))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG, 0x17F9), counter(0xFFE0, True))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0896)
    for n in range(int(os.environ.get('N', 2000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0, 0x0896)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(b"".join(struct.pack("<HB", i, after[i]) for i in range(0x10000) if after[i] != before[i])):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'outinf | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
