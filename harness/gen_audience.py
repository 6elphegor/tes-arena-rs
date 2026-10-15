"""Emit test vectors for the Emperor's audience (src/sidequest.rs
`emperor_audience`, 0000:41A4..4247).

Usage: gen_audience.py ACD_unpacked.EXE OUT

The screens are stubbed, each counting its calls at DGROUP FFE0..FFEC:
1194:DAF0, 2294:0895, 1194:D459, 55B2, 55D5 (twice), DB26 and 0000:9A98;
DAF0's stub instead sets A91A 1234h and D56 5678h, so that the audience
putting them back shows.

A case fills from a seed (`setup`, mirrored in tests/audience.rs) the
words 0D56, A91A, A902, 05E0, 05C8, C398, C39A, the area (5E6), the
player's square (C3D0, C3D4), the keys (652, 12 bytes) and 803C, then
runs 0000:41A4 up to its RETF at 4247.

  audience | seed | result

Results: "stuck", or the FNV-1a digest of the changed DGROUP bytes
(offset, value) with the stubs' counters left out (the port's screens
are three hooks).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter

STUBS = [(0x1194, 0xDAF0), (0x2294, 0x0895), (0x1194, 0xD459), (0x1194, 0x55B2), (0x1194, 0x55D5),
         (0x1194, 0xDB26), (0, 0x9A98)]


def setup(seed):
    """DGROUP writes. Mirrored in tests/audience.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    for at in (0x0D56, 0xA91A, 0xA902, 0x05E0, 0x05C8, 0xC398, 0xC39A, 0xC3D0, 0xC3D4):
        w.append((at, pk(word())))
    w.append((0x05E6, bytes([byte()])))
    w.append((0x0652, bytes(byte() for _ in range(12))))
    w.append((0x803C, bytes([byte()])))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for i, (seg, off) in enumerate(STUBS):
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(0xFFE0 + 2 * i, True))
    # The speech's first screen (DAF0) leaves A91A and D56 changed, as the
    # screens do, so that their putting back shows: mov [A91A], 1234h;
    # mov [D56], 5678h; retf.
    a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, 0xDAF0), bytes.fromhex('c7061aa93412c706560d7856cb'))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_41A4)
    total = int(os.environ.get('N', 1000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.run_range(0, 0x41A4, 0x4247)
            after = bytearray(a.seg_bytes(a.ds))
            for at in range(0xFFE0, 0xFFEE):
                after[at] = before[at]
            res = f'{fnv(changes(before, after)):x}'
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'audience | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
