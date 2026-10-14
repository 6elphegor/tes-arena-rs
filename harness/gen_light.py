"""Emit test vectors for the outdoor light level (src/sky.rs `sky_light`):
0000:0390, the outdoor frame's light, with what it calls stubbed (the
sky's window 2294:0341 and 2294:028C as RETF; 0000:033D, DFDB, 027A,
02F2 and the palette write 9B5B as RET), reporting the level it leaves
at 3C87.

Usage: gen_light.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/light.rs) C398
(bit 4 most of the time), 5C8, F81, the time of day CCFE (mostly near
dawn and dusk) and 3C87.

  light | seed | level

Level: the word at 3C87 afterwards, or "fault" / "hang".
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import mine
from gen_attack import stream


def setup(seed):
    """Writes. Mirrored in tests/light.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0xC398, pk(word() & ~4 | (4 if byte() < 0xE0 else 0))))
    w.append((0x05C8, pk(word())))
    w.append((0x0F81, bytes([byte()])))
    k = byte() % 4
    t = (0xF0 + byte() % 0x30, 0x2E0 + byte() % 0x30, word() & 0x3FF, word())[k]
    w.append((0xCCFE, pk(t)))
    w.append((0x3C87, pk(word())))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x2294, 0x0341, b'\xcb'), (0x2294, 0x028C, b'\xcb'), (0, 0x033D, b'\xc3'), (0, 0xDFDB, b'\xc3'),
                           (0, 0x027A, b'\xc3'), (0, 0x02F2, b'\xc3'), (0, 0x9B5B, b'\xc3')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0390)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        try:
            a.call_near(0, 0x0390)
            res = f'{a.r16(0x3C87):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'light | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
