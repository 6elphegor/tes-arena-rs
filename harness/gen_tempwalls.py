"""Emit test vectors for the changed squares of the wall spells
(src/click.rs `temp_wall_drawn`, 0000:D3F9, and `temp_walls_frame`,
0000:D476) from the original code.

Usage: gen_tempwalls.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/tempwalls.rs) the
list of changed squares (C2C2, 32 words, and their kinds at +40h), the
walls under them and the pause flag (D73C), then for a few frames draws
some squares (0000:D3F9 with DI = the square: listed ones or others)
and runs the frame's reaping (0000:D476).

  temp | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes and the wall plane (32 KB).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from gen_attack import fnv, stream

WSEG = 0x6800


def setup(seed):
    """(writes, frames: lists of squares drawn). Mirrored in
    tests/tempwalls.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    squares = []
    for i in range(0x20):
        if byte() < 0x60:
            w.append((None, 0xC2C2 + 2 * i, pk(0xFFFF)))
            continue
        sq = word() & 0x7FFE
        squares.append(sq)
        k = byte()
        kind = (byte() % 0x0F) << 8 | (0 if k < 0x70 else 1 if k < 0xE0 else byte())
        w.append((None, 0xC2C2 + 2 * i, pk(sq)))
        w.append((None, 0xC302 + 2 * i, pk(kind)))
        w.append((WSEG, sq, pk(word())))
    w.append((None, 0xD73C, bytes([1 if byte() < 0x20 else 0])))
    frames = []
    for _ in range(1 + byte() % 20):
        drawn = []
        for _ in range(byte() % 6):
            drawn.append(squares[byte() % len(squares)] if squares and byte() < 0xE0 else word() & 0x7FFE)
        frames.append(drawn)
    return w, frames


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0xD806, WSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_D476)
    for n in range(int(os.environ.get('N', 2000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        w, frames = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            for drawn in frames:
                for sq in drawn:
                    a.call_near(0, 0xD3F9, edi=sq)
                a.call_near(0, 0xD476)
            after = a.seg_bytes(a.ds)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'temp | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
