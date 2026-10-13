"""Emit test vectors for the player's physics frame (src/movement.rs
`physics_frame`, 0000:F57C, with the fall 0000:F9C4) from the original
code.

Usage: gen_physics.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/physics.rs) the
player's position, height and fall speed, the wall and floor words of
the player's square, the floor kind threshold (CCCE), the flags the fall
tests, the bobbing step, a trigger, and runs N frames of 0000:F57C. The
input and transition hooks (C3B4, C3AC) point at a RETF (23BA:002A,
patched) and the view refresh 0000:DA8E and the view window's save and
restore around the transition (0000:A606, A5D1: the view's rectangle and
centre C3D6/C3D8) are RETs: the port's NoPhysicsHooks do nothing, and
the view is the renderer's.

  physics | seed | result

Results: "fault" / "hang", or an FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from gen_attack import fnv, stream

WSEG, FSEG, FRAME = 0x6800, 0x7000, 0x8800


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes and the frame
    count. Mirrored in tests/physics.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    px, py = word() & 0x1FFF, word() & 0x1FFF
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    k = byte()
    height = 0x3C if k < 0x60 else 0x3C + byte() if k < 0xA0 else (byte() - 0x80) & 0xFFFF if k < 0xE0 else word()
    w.append((None, 0xC3D2, pk(height)))
    w.append((None, 0xE8BE, pk(4 if byte() < 0x80 else byte() % 0x40)))
    sq = (py >> 7 << 8) + (px >> 7 << 1)
    k = byte()
    if k < 0x50:
        wall = 0
    elif k < 0x80:
        v = byte()
        wall = v << 8 | v
    elif k < 0xE0:
        wall = word() & 0x7FFF
    else:
        wall = word()
    w.append((WSEG, sq, pk(wall)))
    k = byte()
    floor = FRAME + 0xC00 if k < 0x40 else FRAME + 0xC00 + word() % 0x800 if k < 0xA0 else word()
    w.append((FSEG, sq, pk(floor)))
    w.append((None, 0xCCCE, pk(FRAME + 0xC00 if byte() < 0xC0 else word())))
    w.append((None, 0xC39A, pk(word() & 0xC211)))
    w.append((None, 0xC398, pk(word() & 0x2204)))
    w.append((None, 0xE39F, bytes([0 if byte() < 0xC0 else byte() % 4])))
    w.append((None, 0xE8B6, pk(word() & 0xF)))
    w.append((None, 0xCCBC, pk(word())))
    w.append((None, 0xC3D8, pk(word())))
    w.append((None, 0xD80C, pk(word())))
    w.append((None, 0xCB6A, pk(0 if byte() < 0xC0 else word())))
    w.append((None, 0xCB68, pk(word())))
    return w, 1 + byte() % 24


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG + 0x23BA, 0x002A), b'\xcb')
    for off in (0xDA8E, 0xA5D1, 0xA606):
        a.mu.mem_write(a.lin(LOAD_SEG, off), b'\xc3')
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    a.w16(0x9030, FRAME)
    for hook in (0xC3B4, 0xC3AC):
        a.w16(hook, 0x002A)
        a.w16(hook + 2, LOAD_SEG + 0x23BA)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = __import__('random').Random(0x1994_F57C)
    for n in range(3000):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, FSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, frames = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            for _ in range(frames):
                a.call_near(0, 0xF57C)
            after = a.seg_bytes(a.ds)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'physics | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
