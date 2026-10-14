"""Emit test vectors for the outdoor frame's light (src/light.rs
`outdoor_light`, 0000:0390, with the night and day switches 2294:028C,
0000:033D, the light table's reset 2294:0B8A, the sky's window
2294:0341 and the level's lights 0000:0810 running).

Usage: gen_outlight.py ACD_unpacked.EXE OUT

Stubbed, counting their calls in DGROUP: 0000:6C32 (FFE0), 7428 (FFE2),
AAA2 (FFE4), DFDB (FFE6), 027A (FFE8), 02F2 (FFEA), 9B5B (FFEC),
2294:08B9 (FFEE), 0000:9B52 (FFF0; the .COL's segment 43C0 is set to
DGROUP so it lands there).

A case fills from a seed (`setup`, mirrored in tests/outlight.rs) C398
(bit 4 mostly, bit 8000h at random), C39A (bit 200h, the wild, now and
then), 5C8 (bits 200h, 400h), fog (F81, F82), the time of day (around
dawn, dusk, night or day), the light level (3C87), the row limits, the
sky's records (3C85..3C8B), the near list's key, a small map (lit and
unlit lamps, 801Dh and 801Eh, other flats, walls) and its floor plane,
the flats' light bytes and the first flat, and the level's lights.

  outlight | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes and the wall plane.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes
from emu import mine
from gen_attack import fnv, stream
from gen_frame import counter

WSEG = 0x6800
FSEG = 0x7800


def setup(seed):
    """Writes. Mirrored in tests/outlight.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    w.append((None, 0xC398, pk(word() & 0x7FFB | (4 if byte() < 0xE0 else 0) | (0x8000 if byte() & 1 else 0))))
    w.append((None, 0xC39A, pk(word() & ~0x200 | (0x200 if byte() < 0x40 else 0))))
    w.append((None, 0x05C8, pk(word())))
    w.append((None, 0x0F81, pb(0 if byte() < 0xC0 else byte())))
    w.append((None, 0x0F82, pb(byte())))
    k = byte() % 5
    t = (0xF8 + byte() % 0x20, 0x2E8 + byte() % 0x20, byte() % 0x100, 0x300 + byte(), 0x110 + word() % 0x1E0)[k]
    w.append((None, 0xCCFE, pk(t)))
    w.append((None, 0x3C87, pk((0xA00, 0xFFFF, (byte() % 11) << 8, word())[byte() % 4])))
    for at in (0xC8E7, 0xC8E9, 0xC8EB):
        w.append((None, at, pk(byte() % 16)))
    for at in (0x3C85, 0x3C89, 0x3C8B):
        w.append((None, at, pk(word())))
    w.append((None, 0xC8ED, pk(word())))
    cols, rows = 1 + byte() % 12, 1 + byte() % 12
    w.append((None, 0xC357, pk(cols)))
    w.append((None, 0xC359, pk(rows)))
    for r in range(rows):
        for c in range(cols):
            sq = r << 8 | c << 1
            k = byte() % 8
            if k < 2:
                w.append((WSEG, sq, pk(0x801D + k)))
            elif k == 2:
                w.append((WSEG, sq, pk(0x8000 | byte() % 0x20)))
            elif k == 3:
                w.append((FSEG, sq, pk(byte() << 8 | 1 + byte() % 0x20)))
            elif k == 4:
                w.append((WSEG, sq, pk(word())))
    w.append((None, 0xC38E, pk(byte() % 0x10)))
    for f in range(0x40):
        w.append((None, 0xCDA2 + 5 * f, pb(0 if byte() < 0xA0 else 1 + byte() % 8)))
    n = byte() % 8
    w.append((None, 0xBC8E, pk(n)))
    for i in range(n * 3):
        w.append((None, 0xBC90 + 2 * i, pk(word())))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, at, far in [(0, 0x6C32, 0xFFE0, True), (0, 0x7428, 0xFFE2, True), (0, 0xAAA2, 0xFFE4, True),
                              (0, 0xDFDB, 0xFFE6, False), (0, 0x027A, 0xFFE8, False), (0, 0x02F2, 0xFFEA, False),
                              (0, 0x9B5B, 0xFFEC, False), (0x2294, 0x08B9, 0xFFEE, True), (0, 0x9B52, 0xFFF0, True)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(at, far))
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    a.w16(0x43C0, a.ds)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_0390)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        a.mu.mem_write(a.lin(FSEG, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0, 0x0390)
            after = a.seg_bytes(a.ds)
            ch = changes(before, after)
            res = f'{fnv(ch + a.seg_bytes(WSEG)[:0x1000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'outlight | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
