"""Emit test vectors for a swing hitting the wall ahead (src/weapon.rs
`bash`, 0000:3D43) from the original code.

Usage: gen_bash.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/bash.rs) the
player's position, heading and square (CB90), the square ahead and its
neighbours (plain walls, trigger walls, breakable flats C93B, doors with
locks), the triggers (C917) and the trigger kinds' tables, the city and
level flags, strength, level and damage modifier, the guards' state
(0FFB, 5E6, D48), the actor slots' flags and the effect slots (calmed by
the guards), and the random seed; then runs 0000:3D43 once. The player
fights bare-handed (no items).

Stubbed: the sound 23BA:002A (RETF); the wall trigger 0000:43B1 counts
its calls at DGROUP FFF4 (RETF); whether a building is closed now
(1194:4FB8) is DGROUP FFF8 bit 0 (the port's test hooks do the same).

  bash | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes and the wall plane (32 KB).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from gen_attack import fnv, stream

WSEG = 0x6800
# 1194:4FB8: test byte [FFF8],1; jz +2; stc; retf; clc; retf
CLOSED = bytes.fromhex('f606f8ff017402f9cbf8cb')
# 0000:43B1: inc word [FFF4]; retf
TRIGGER = bytes.fromhex('ff06f4ffcb')


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes. Mirrored in
    tests/bash.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    px, py = 0x200 + word() % 0x3C00, 0x200 + word() % 0x3C00
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    w.append((None, 0xC3DA, pk(word() & 0x1FF)))
    sq = (py >> 7 << 8) + (px >> 7 << 1)
    w.append((None, 0xCB90, pk(sq)))
    triggers = bytes(byte() % 0x40 for _ in range(16))
    w.append((None, 0xC917, triggers))
    flat = byte()
    w.append((None, 0xC93B, pb(flat)))
    w.append((None, 0xC93C, pb(byte())))
    locks = []
    for dy in (-0x100, 0, 0x100):
        for dx in (-2, 0, 2):
            at = (sq + dy + dx) & 0xFFFF
            k = byte()
            if k < 0x30:
                wall = 0
            elif k < 0x80:
                t = triggers[byte() % 16] if byte() < 0xC0 else byte() % 0x40
                wall = t * 0x101 | (0 if byte() < 0xE0 else word() & 0x7F00)
            elif k < 0xB0:
                wall = 0x8000 | word() & 0x0F00 | (flat if byte() < 0xC0 else byte())
            elif k < 0xF0:
                wall = 0xB000 | word() & 0x0DFF
                if byte() < 0xC0:
                    locks.append((at, byte() % 0x20))
            else:
                wall = word()
            w.append((WSEG, at, pk(wall)))
    w.append((None, 0xC6E4, pb(len(locks))))
    for i, (at, level) in enumerate(locks):
        w.append((None, 0xC624 + 3 * i, pk(at & 0xFF00 | (at & 0xFF) >> 1) + pb(level)))
    w.append((None, 0xC398, pk(4 if byte() < 0xA0 else 0)))
    w.append((None, 0xC39A, pk(word() & 0x0200)))
    w.append((None, 0xFFF8, pb(byte() & 1)))
    w.append((None, 0x01CD, pb(byte())))
    w.append((None, 0x01A4 + 6, pb(byte() % 20)))
    w.append((None, 0x01A4 + 0x39, pk(byte() % 16)))
    w.append((None, 0x05CA, pk(word() & 0x0820)))
    w.append((None, 0x0FFB, pk(0 if byte() < 0xC0 else word())))
    w.append((None, 0x0FF9, pk(word())))
    w.append((None, 0x05E6, pb(4 if byte() < 0x80 else byte() % 6)))
    w.append((None, 0x0D48, pb(byte() % 12)))
    for i in range(23):
        w.append((None, 0x0AB3 + 0x1C * i + 0xF, pk(word())))
        w.append((None, 0x0AB3 + 0x1C * i + 0x12, pk(0x41E * (i % 8))))
    w.append((None, 0x0660, bytes(byte() for _ in range(16 * 0x3C))))
    for i in range(16):
        if byte() < 0x60:
            w.append((None, 0x0660 + 0x3C * i + 0x28, pk(0x41E * (byte() % 8)) + pk(0x7800)))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    return w


def unrelocated(mem):
    """The guards' hook segment (67EC) image-relative."""
    mem = bytearray(mem)
    if struct.unpack_from('<H', mem, 0x67EC)[0] == LOAD_SEG:
        struct.pack_into('<H', mem, 0x67EC, 0)
    return bytes(mem)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x23BA, 0x002A, b'\xcb'), (0, 0x43B1, TRIGGER), (0x1194, 0x4FB8, CLOSED)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0xD806, WSEG)
    a.w16(0x1042, 0x7800)
    a.w16(0x91F0, 0)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_3D43)
    for n in range(int(os.environ.get('N', 3000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            a.call_near(0, 0x3D43)
            after = unrelocated(a.seg_bytes(a.ds))
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'bash | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
