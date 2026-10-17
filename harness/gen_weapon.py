"""Emit test vectors for the player's weapon each frame (src/weapon.rs
`weapon_frame`, 0000:38CE) from the original code.

Usage: gen_weapon.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/weapon.rs) the
hand's frame (1947), the flags (5E0: armed 10h, swinging 4, cursor in
the centre 40h), the buttons, the weapon classes (D49, 1012), the bow's
wait, the casting hands' frame and their graphic's frame count, a spell
waiting for a target, the level flags, agility, the player away from
the actors (so a landing swing reaches none), and per frame the mouse's
motion; then runs N frames of 0000:38CE.

The mouse driver's motion counters (INT 33h function 0Bh) return the
frame's motion and clear it. The hand drawing 0000:3B8E (RET), the
casting hands' bank switches and drawing (0000:5C86, 78A8: RETF; 5C8A:
RET) are the renderer's, the sound 23BA:002A a RETF. The hands' frame is
unpacked (5B3C) as the game does; with no EMS here the frame lands on
the graphic itself, as it does in the port's test. The wall bash runs
on an empty wall plane with gen_bash.py's stubs; the bow 29CF:037F
counts its calls at DGROUP FFF6 (the port's test hooks do the same).

  weapon | seed | result

Results: "fault" / "hang", or an FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_CX, UC_X86_REG_DX
from emu import Arena, Fault, LOAD_SEG
from emu import changes
from emu import mine
from gen_attack import fnv, stream
from gen_bash import TRIGGER, CLOSED, WSEG, unrelocated

HSEG, BUFSEG = 0x6000, 0x6800


class MouseArena(Arena):
    """Arena with the mouse driver's motion counters (INT 33h, 0Bh)."""
    motion = (0, 0)

    def _intr(self, mu, intno, data):
        if intno == 0x33 and mu.reg_read(UC_X86_REG_AX) == 0x0B:
            mu.reg_write(UC_X86_REG_CX, self.motion[0] & 0xFFFF)
            mu.reg_write(UC_X86_REG_DX, self.motion[1] & 0xFFFF)
            self.motion = (0, 0)
            return
        super()._intr(mu, intno, data)


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes, and the
    motion of each frame. Mirrored in tests/weapon.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    k = byte()
    frame = byte() % 0x1E if k < 0x80 else 0x1E + byte() % 6 if k < 0xC0 else 0x20 if k < 0xF0 else word()
    w.append((None, 0x1947, pk(frame)))
    w.append((None, 0x05E0, pk(word() & 0x0054 | (0x10 if byte() < 0xC0 else 0))))
    w.append((None, 0xAFF9, pb(byte() & 3)))
    k = byte()
    w.append((None, 0x1012, pb(6 if k < 0x30 else 7 + byte() % 3 if k < 0x60 else byte() % 6)))
    w.append((None, 0x0D49, pb(byte() if byte() < 0x20 else byte() % 0x10)))
    w.append((None, 0x680E, pb(0 if byte() < 0xA0 else byte() % 8)))
    w.append((None, 0x7F80, pb(0xFF if byte() < 0xC0 else byte() % 12)))
    w.append((HSEG, 0x0B, pb(byte() % 10)))
    w.append((None, 0x0620, pk(0 if byte() < 0xF0 else word())))
    w.append((None, 0xC39A, pk(0 if byte() < 0xE0 else word() & 0x4010)))
    w.append((None, 0x05B1, pk(0x40 if byte() < 0x10 else 0)))
    w.append((None, 0x05CA, pk(0x10 if byte() < 0x10 else 0)))
    w.append((None, 0x01D1, pb(byte())))
    w.append((None, 0xC3D0, pk(0x800 + word() % 0x4000)))
    w.append((None, 0xC3D4, pk(0x800 + word() % 0x4000)))
    w.append((None, 0xC3DA, pk(word() & 0x1FF)))
    w.append((None, 0xC3D2, pk(0x3C if byte() < 0xE0 else 0)))
    motions = []
    for _ in range(1 + byte() % 12):
        k = byte()
        if k < 0x60:
            motions.append((0, 0))
        elif k < 0xE0:
            motions.append(((byte() - 0x80) * 2, (byte() - 0x80) * 2))
        else:
            motions.append((word(), word()))
    return w, motions


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MouseArena(EXE)
    for seg, off, code in [(0, 0x3B8E, b'\xc3'), (0, 0x5C8A, b'\xc3'), (0, 0x5C86, b'\xcb'),
                           (0, 0x78A8, b'\xcb'), (0x23BA, 0x002A, b'\xcb'),
                           (0, 0x43B1, TRIGGER), (0x1194, 0x4FB8, CLOSED),
                           (0x29CF, 0x037F, bytes.fromhex('ff06f6ffcb'))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0x902E, 0)
    a.w16(0x9030, HSEG)
    a.w16(0xBA54, 0)
    a.w16(0xBA56, BUFSEG)
    a.w16(0xD806, WSEG)
    a.w16(0x1042, 0x7800)
    a.w16(0x91F0, 0)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_38CE)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (HSEG, BUFSEG, WSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, motions = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            for m in motions:
                a.motion = m
                a.call_near(0, 0x38CE)
            after = unrelocated(a.seg_bytes(a.ds))
            changed = changes(before, after)
            res = f'{fnv(changed):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'weapon | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
