"""Emit test vectors for a wall's trigger (src/trigger.rs
`wall_trigger`, 0000:43B1, with its handlers and the building
entry 0000:4054) from the original code.

Usage: gen_trigger.py ACD_unpacked.EXE OUT

Stubbed, counting their calls in DGROUP: 1194:59C8 (FFE0), 23BA:002A
(FFE2), 1194:C048 (FFE4), the palace's audience 0000:41A4 (FFE6),
1194:A36C (FFE8), 2F9F:0438 (FFEA), 1194:55B2 (FFEC), 23F1:0178 (FFEE,
near), 217B:04DC (FFF0), 217B:027A (FFF2), 0000:4EFD (FFF4), 0000:946A
(FFF6, near), 1194:D55F (FFF8), 2433:0199 (FFFA), 0000:3EBE (FFFC).

A case fills from a seed (`setup`, mirrored in tests/trigger.rs) the
player's place, the square under the cursor (CB92) near it and its wall
(a trigger's, plain or with bit 8000h, or another), the triggers
(C917), the map's flags (indoors, city, wilderness), the city (A46,
A47, A53) and the main quest's stage (F77), the break-in and picking
bits (5CA), an Open spell (DC4), the time and date (for the hours), the
lockpicking stats and failed picks, the message showing, the scratch
block (8FAA) and the random seed; EAX's high word, last, varied to
show the distance doesn't read it (0000:122A's 99h is CWD in 16-bit
code, though capstone prints CDQ). Then 0000:43B1 is far-called.

  trigger | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes (the message's segment image-relative).
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
from gen_input import unrelocated

WSEG = 0x6800

STUBS = [
    (0x1194, 0x59C8, 0xFFE0, True), (0x23BA, 0x002A, 0xFFE2, True), (0x1194, 0xC048, 0xFFE4, True),
    (0, 0x41A4, 0xFFE6, True), (0x1194, 0xA36C, 0xFFE8, True), (0x2F9F, 0x0438, 0xFFEA, True),
    (0x1194, 0x55B2, 0xFFEC, True), (0x23F1, 0x0178, 0xFFEE, False), (0x217B, 0x04DC, 0xFFF0, True),
    (0x217B, 0x027A, 0xFFF2, True), (0, 0x4EFD, 0xFFF4, True), (0, 0x946A, 0xFFF6, False),
    (0x1194, 0xD55F, 0xFFF8, True), (0x2433, 0x0199, 0xFFFA, True), (0, 0x3EBE, 0xFFFC, True),
]


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes, and EAX's
    high word. Mirrored in tests/trigger.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    col = 4 + byte() % 24
    row = 4 + byte() % 24
    px = col << 7 | byte() & 0x7F
    py = row << 7 | byte() & 0x7F
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    k = byte()
    if k < 0x10:
        sq = 0xFFFF
    elif k < 0x18:
        sq = word()
    else:
        r = row + byte() % 3 - 1
        c = col + byte() % 3 - 1
        sq = r << 8 | c << 1
    w.append((None, 0xCB92, pk(sq)))
    trig = [byte() % 0x40 if byte() < 0xC0 else byte() for _ in range(16)]
    i = byte() % 16
    t = byte() % 0x40
    trig[i] = t
    w.append((None, 0xC917, bytes(trig)))
    k = byte()
    if k < 0x80:
        wall = t << 8 | t
    elif k < 0xA0:
        wall = 0x8000 | byte() << 8 | t | byte() & 0xC0
    elif k < 0xB0:
        wall = (t << 8 | t) ^ 0x40
    elif k < 0xC0:
        wall = t << 8 | byte()
    else:
        wall = word()
    if sq != 0xFFFF:
        w.append((WSEG, sq, pk(wall)))
    k = byte()
    outdoors = 4 if k < 0xC0 else 0
    w.append((None, 0xC398, pk(word() & 0xFF7B | outdoors)))
    v = word() & 0xFDEF
    if byte() < 0x60:
        v |= 0x200
    if byte() < 0x10:
        v |= 0x10
    w.append((None, 0xC39A, pk(v)))
    w.append((None, 0x05C8, pk(word())))
    w.append((None, 0x05E6, pb(byte())))
    w.append((None, 0x0D48, pb(byte())))
    w.append((None, 0x0A46, pb(8 if byte() < 0x80 else byte() % 10)))
    w.append((None, 0x0A47, pb(byte() % 3)))
    w.append((None, 0x0A53, pk(word())))
    k = byte()
    w.append((None, 0x0F77, pb(8 if k < 0x40 else 9 if k < 0x80 else byte() % 12)))
    v = word() & 0xF7FE
    if byte() < 0x30:
        v |= 0x800
    if byte() < 0x80:
        v |= 1
    w.append((None, 0x05CA, pk(v)))
    k = byte()
    w.append((None, 0x0DC4, pk(0 if k < 0x80 else 0x7D0 if k < 0xA0 else byte() % 0x60)))
    w.append((None, 0x01AA, pb(byte() % 0x20)))
    w.append((None, 0xCCFE, pk(word() % 0x400)))
    w.append((None, 0x05CC, pk(word()) + pk(word() % 4)))
    for o in (0x2A, 0x2C):
        w.append((None, 0x01A4 + o, pb(byte())))
    w.append((None, 0x01A4 + 5, pb(byte() % 0x12)))
    w.append((None, 0x01A4 + 6, pb(byte() % 30)))
    w.append((None, 0x0F88, b''.join(pk(sq if byte() < 0x20 else 0xFFFF if byte() < 0xC0 else word()) for _ in range(0x20))))
    w.append((None, 0x061E, pk(byte() % 3)))
    w.append((None, 0x061A, pk(0x7BFA if byte() < 0x80 else word())))
    for o in (0x1036, 0x102C, 0xCB90):
        w.append((None, o, pk(word())))
    w.append((None, 0x8FAA, bytes(byte() for _ in range(0x40))))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    w.append((None, 0x0FF7, pk(word())))
    w.append((None, 0x6636, pb(byte())))
    k = byte()
    high = 0 if k < 0x60 else 0xFFFF if k < 0xC0 else word()
    return w, high


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, at, far in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(at, far))
    a.w16(0xD806, WSEG)
    a.w16(0x91F0, 0)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_43B1)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        w, high = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before = unrelocated(a, a.seg_bytes(a.ds))
        try:
            a.call_far(0, 0x43B1, eax=high << 16 | 0x1234, fs=a.ds, gs=a.ds)
            after = unrelocated(a, a.seg_bytes(a.ds))
            ch = changes(before, after)
            res = f'{fnv(ch):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'trigger | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
