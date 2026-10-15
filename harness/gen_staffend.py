"""Emit test vectors for the staff made whole (src/staffend.rs
`staff_end`, 1194:D986..DAEF).

Usage: gen_staffend.py ACD_unpacked.EXE OUT

The screens are stubbed, each counting its calls at DGROUP FFC0..FFD8:
1194:DAF0, 2294:0895, 1194:D459, 1194:55B2, 0000:9426, 0000:9466,
0000:5C86, 0000:7428, 1194:A36C, 0000:5364, 1194:5532, 1194:DB26,
0000:9A98; 0000:5828 and 9B4E (called with DS the EMS frame's) are a
bare RETF. DAF0's stub also sets A91A 1234h and
D56 5678h, so that their putting back shows; 5364's adds the line's SI
and row (BX) into FFDA/FFDC (wrapping), so that the lines drawn show.
0000:9AE6 (the font) runs as it is.

A case fills from a seed (`setup`, mirrored in tests/staffend.rs) the
words 0D56, A91A, A902, 102C, 137A, the bytes 9288, 803C, 101C, 137E,
and the house's text at 929E: a run of bytes, some newlines (0Ah), most
cases ended by a NUL (the rest by whatever DGROUP holds after it); then
runs 1194:D986 to its RETF.

  staffend | seed | counts | result

counts: the stubs' calls, then the two sums, as hex words joined by
'.'; result: "stuck", or the FNV-1a digest of the changed DGROUP bytes
(offset, value) with the stubs' words left out.
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

STUBS = [(0x1194, 0xDAF0), (0x2294, 0x0895), (0x1194, 0xD459), (0x1194, 0x55B2), (0, 0x9426), (0, 0x9466),
         (0, 0x5C86), (0, 0x7428), (0x1194, 0xA36C), (0, 0x5364),
         (0x1194, 0x5532), (0x1194, 0xDB26), (0, 0x9A98)]
COUNTS = 0xFFC0
SUMS = 0xFFDA


def setup(seed):
    """DGROUP writes. Mirrored in tests/staffend.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    for at in (0x0D56, 0xA91A, 0xA902, 0x102C, 0x137A):
        w.append((at, pk(word())))
    for at in (0x9288, 0x803C, 0x101C, 0x137E):
        w.append((at, bytes([byte()])))
    n = byte() % 120
    text = bytearray()
    for _ in range(n):
        b = byte()
        text.append(0x0A if b < 0x30 else 0x20 + b % 0x5F)
    if byte() >= 0x20:
        text.append(0)
    w.append((0x929E, bytes(text)))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for i, (seg, off) in enumerate(STUBS):
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(COUNTS + 2 * i, True))
    # DAF0: mov [A91A], 1234h; mov [D56], 5678h; inc word [FFC0]; retf.
    a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, 0xDAF0), bytes.fromhex('c7061aa93412c706560d7856') + counter(COUNTS, True))
    for off in (0x5828, 0x9B4E):
        a.mu.mem_write(a.lin(LOAD_SEG, off), b'\xcb')
    # 5364: add [FFDA], si; add [FFDC], bx; inc word [FFD2]; retf.
    at = COUNTS + 2 * STUBS.index((0, 0x5364))
    a.mu.mem_write(a.lin(LOAD_SEG, 0x5364), bytes.fromhex('0136daff011edcff') + counter(at, True))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_D986)
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
            a.call_far(0x1194, 0xD986)
            after = bytearray(a.seg_bytes(a.ds))
            counts = '.'.join(f'{struct.unpack_from("<H", after, COUNTS + 2 * i)[0]:x}' for i in range(len(STUBS) + 2))
            for i in range(COUNTS, SUMS + 4):
                after[i] = before[i]
            res = f'{fnv(changes(before, after)):x}'
        except (Fault, AssertionError):
            counts, res = '-', 'stuck'
        out.write(f'staffend | {seed:x} | {counts} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
