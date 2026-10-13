"""Emit test vectors for a click in the view (src/click.rs `view_click`,
2826:077E) from the original code.

Usage: gen_click.py ACD_unpacked.EXE OUT

A case is gen_input.py's setup followed (from the same byte stream; see
`setup`, mirrored in tests/click.rs) by what a click meets: the wall
square under the cursor (CB92), the floor square (DGROUP FFF0, read by
the patched 2826:0B52), doors around the player and the door drawn
under the cursor (CCB2..CCB8, CCC8), locks (C624, count C6E4), the
player's keys (0652), the failed picks (F88), the lockpicking stats, a
spell waiting for a target (walls, floors, Open, Lock, or a cast at
0620 through a far pointer at 1044), the actors' squares (1951), the
changed squares (C2C2) and the random seed; then 2826:077E runs once.
The stubs are gen_input.py's.

  click | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes, the wall plane and the floor plane (32 KB each).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from gen_attack import fnv, stream
import gen_input as gi

SSEG = 0x6000


def setup(seed):
    s = stream(seed)
    w, _ = gi.setup(s)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    get = {off: data for seg, off, data in w if seg is None}
    px, py = struct.unpack('<H', get[0xC3D0])[0], struct.unpack('<H', get[0xC3D4])[0]
    col, row = px >> 7, py >> 7

    def near():
        return (row + byte() % 5 - 2) & 0xFF, (col + byte() % 5 - 2) & 0xFF

    def square(r, c):
        return (r << 8) + (c << 1) & 0xFFFF

    # Doors: a few squares around the player.
    doors = []
    for _ in range(byte() % 6):
        r, c = near()
        doors.append((r, c))
        wall = 0xB000 | word() & 0x01FF | (0 if byte() < 0xC0 else word() & 0x0E00)
        w.append((gi.WSEG, square(r, c), pk(wall)))
    # The door drawn under the cursor.
    r, c = doors[byte() % len(doors)] if doors and byte() < 0xC0 else near()
    x0 = word() % 0x150
    w.append((None, 0xCCB2, pk(x0 - 0x20)))
    w.append((None, 0xCCB4, pk(x0 + byte())))
    w.append((None, 0xCCB6, pk(c)))
    w.append((None, 0xCCB8, pk(r)))
    w.append((None, 0xCCC8, pk(square(r, c) if byte() < 0xE0 else word())))
    # The wall and the floor under the cursor.
    k = byte()
    if k < 0x30:
        picked = 0xFFFF
    elif k < 0xC0 and doors:
        picked = square(*doors[byte() % len(doors)])
    elif k < 0xF0:
        picked = square(*near())
    else:
        picked = word()
    w.append((None, 0xCB92, pk(picked)))
    w.append((None, 0xFFF0, pk(0xFFFF if byte() < 0x40 else square(*near()))))
    # Locks: on the doors, or anywhere.
    n = byte() % 8
    w.append((None, 0xC6E4, pb(n)))
    for i in range(n):
        r, c = doors[byte() % len(doors)] if doors and byte() < 0xC0 else near()
        k = byte()
        level = 1 + byte() % 12 if k < 0x80 else byte() % 0x20 if k < 0xF0 else byte()
        w.append((None, 0xC624 + 3 * i, pk(r << 8 | c) + pb(level)))
    w.append((None, 0x0652, bytes(0xC5 if byte() < 0x80 else 1 + byte() % 12 for _ in range(12))))
    w.append((None, 0x0F88, b''.join(pk(picked if byte() < 0x10 else 0xFFFF) for _ in range(0x20))))
    # Lockpicking: the player's stats, the class factors are the game's.
    w.append((None, 0x01A4 + 0x2A, pb(byte())))
    w.append((None, 0x01A4 + 0x2C, pb(byte())))
    w.append((None, 0x01A4 + 5, pb(byte() % 0x12)))
    w.append((None, 0x01A4 + 6, pb(byte() % 30)))
    w.append((None, 0x12BC, pb(0 if byte() < 0xC0 else 1)))
    w.append((None, 0xCB5B, pb(byte())))
    # A spell waiting for a target.
    for o in (0x0DC0, 0x0DC2, 0x0DC4, 0x0DC8, 0x0620):
        w.append((None, o, pk(0)))
    k = byte() % 7
    if k == 1:
        w.append((None, 0x0DC0, pk(-(1 + byte() % 3) if byte() < 0x80 else 1 + byte() % 3)))
    elif k == 2:
        w.append((None, 0x0DC2, pk(-(1 + byte() % 3) if byte() < 0x80 else 1 + byte() % 3)))
    elif k == 3:
        w.append((None, 0x0DC4, pk(0x7D0 if byte() < 0x60 else byte())))
    elif k == 4:
        w.append((None, 0x0DC8, pk(1 + byte() % 0x20)))
    elif k == 5:
        w.append((None, 0x0620, pk(0x1044)))
        w.append((None, 0x1044, pk(0) + pk(SSEG)))
        w.append((SSEG, 0, bytes(byte() for _ in range(0x30))))
    w.append((None, 0x0DC6, pk(byte())))
    # Actors' squares and the changed squares.
    w.append((None, 0x1951, b''.join(pk(square(*near()) if byte() < 0x20 else 0xFFFF) for _ in range(0x17))))
    w.append((None, 0xC2C2, b''.join(pk(0xFFFF if byte() < 0xC0 else word()) for _ in range(0x20))))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    gi.prepare(a)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_077E)
    for n in range(int(os.environ.get('N', 3000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (gi.WSEG, gi.FSEG, gi.CURSEG, SSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = gi.unrelocated(a, a.seg_bytes(a.ds))
        try:
            a.call_near(0x2826, 0x077E)
            after = gi.unrelocated(a, a.seg_bytes(a.ds))
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(gi.WSEG)[:0x8000]):x} {fnv(a.seg_bytes(gi.FSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'click | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
