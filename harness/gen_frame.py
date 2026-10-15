"""Emit test vectors for the world loop's frame body (src/frame.rs
`frame_body`, 0000:481E up to the detail wait at 4CE7) from the original
code.

Usage: gen_frame.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/frame.rs) the keys
and the keys already reported, the input and speed state, the area and
level flags and the wilderness flats, the spell flags and the light's
timer, the weapon and hands, the message, the speed stats and a fight's
creature slots, the guards' countdown, the hold and the water, the date
(some on a holiday), night and the night creatures' conditions, a town
entered, a pending message box, the timer ticks, the hurt flash and the
random seed; then runs 0000:481E to 4CE7.

Stubbed as the port's FrameHooks (counting calls at DGROUP FFE0..FFEE
where the hook is observable) or as drawing, sound and menus (RET/RETF):
see STUBS. The weapon's stubs are gen_weapon.py's (the mouse's motion
counters read 0), the clock's gen_clock.py's.

  frame | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes, the wall plane and the creature segment (32 KB each).
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
from gen_bash import TRIGGER, CLOSED
import gen_bash
import gen_input
from gen_weapon import MouseArena

WSEG, CSEG, TSEG, HSEG = 0x6800, 0x7800, 0x5C00, 0x6000


def counter(at, far):
    """inc word [at]; ret/retf"""
    return bytes([0xFF, 0x06, at & 0xFF, at >> 8, 0xCB if far else 0xC3])


RETF, RET = b'\xcb', b'\xc3'
STUBS = [
    # Drawing, sound, menus, message boxes.
    (0x2294, 0x11F2, RETF), (0x1194, 0x4635, RETF), (0x2294, 0x04F5, RETF), (0x1194, 0xA36C, counter(0xFFE0, True)),
    (0, 0x7428, RETF), (0, 0x599C, RETF), (0, 0x5104, RETF), (0, 0x05B5, counter(0xFFD0, False)), (0x292B, 0x05A2, counter(0xFFD2, True)),
    (0x2542, 0x0100, counter(0xFFE2, True)),
    (0x1194, 0xDB6C, counter(0xFFE4, True)), (0x1194, 0x3A50, RETF), (0x25FD, 0x0844, RETF), (0x1194, 0x5532, RETF),
    (0x1194, 0xD327, counter(0xFFE6, True)), (0, 0x4F72, RETF), (0x2294, 0x0428, RETF), (0x292B, 0x0530, RETF),
    (0, 0x5A66, RETF), (0x292B, 0x000E, RETF), (0x1194, 0x59C8, counter(0xFFE8, True)), (0x23BA, 0x002A, RETF),
    (0, 0x4D2F, RET), (0, 0x5368, RET), (0, 0x4D7C, RET), (0, 0x3491, RET), (0, 0x5C8A, RET), (0, 0x965E, RET),
    (0, 0x946A, RET), (0, 0x0390, RET), (0, 0x5011, RET), (0, 0x4DD6, RET), (0, 0x742C, RET),
    # Hooks with a visible effect.
    (0, 0x3F0C, counter(0xFFEA, True)), (0, 0x3F3B, counter(0xFFEC, True)), (0, 0x17F9, counter(0xFFEE, True)),
    # 0000:4D00: test byte [FFDE],1; jz +2; stc; ret; clc; ret
    (0, 0x4D00, bytes.fromhex('f606deff017402f9c3f8c3')),
    # The weapon's (gen_weapon.py) and the clock's (gen_clock.py).
    (0, 0x3B8E, RET), (0, 0x5C86, RETF), (0, 0x5B3C, RETF), (0, 0x78A8, RETF), (0, 0x43B1, TRIGGER),
    (0x1194, 0x4FB8, CLOSED), (0x29CF, 0x037F, counter(0xFFF6, True)),
    (0x1194, 0x99F8, RETF), (0x1194, 0x98E8, RETF), (0x1194, 0x887A, RETF), (0x1194, 0x86F2, RETF),
    (0x1194, 0x1034, RETF), (0, 0x9AF8, RETF), (0x2901, 0x0014, RETF),
]
KEYS = [0x0D, 0x0C, 0x3E, 0x3C, 0x3D, 0x31, 0x38, 0x3F, 0x30, 0x10, 0x2D]


def setup(seed, holidays):
    """(segment or None for DGROUP, offset, bytes) writes. Mirrored in
    tests/frame.rs. `holidays` are the days of the table at 33AD."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    for scan in KEYS:
        w.append((None, 0xACB5 + scan, pb(1 if byte() < 0x30 else 0)))
        w.append((None, 0x9208 + scan, pb(1 if byte() < 0x40 else 0)))
    w.append((None, 0x37E9, pb(byte() % 11)))
    w.append((None, 0xAFF9, pb(byte() & 3)))
    w.append((None, 0x05E3, pb(0 if byte() < 0xC0 else 1)))
    w.append((None, 0x7A46, pk(word() & 0x1FF)))
    w.append((None, 0x05CA, pk(word() & 0x721E)))
    w.append((None, 0x05E0, pk(word() & 0x2554)))
    w.append((None, 0xC39A, pk(word() & 0x4211)))
    w.append((None, 0x8848, pk(word() & 0x0010)))
    k = byte()
    w.append((None, 0xC942, pb(0xFF if k < 0x20 else byte() % 8)))
    w.append((None, 0x05BE, pk(word() & 0x0184)))
    w.append((None, 0x1020, pk(0) + pk(TSEG)))
    w.append((TSEG, 0x22, pk(byte() % 8)))
    w.append((None, 0x01A9, pb(byte())))
    w.append((None, 0x0D49, pb(byte() % 10 if byte() < 0xE0 else 0xFF)))
    w.append((None, 0x1012, pb(byte() % 10)))
    w.append((None, 0x7F80, pb(0xFF if byte() < 0xE0 else byte() % 8)))
    w.append((None, 0x1947, pk(0x20 if byte() < 0x80 else byte() % 0x24)))
    w.append((None, 0x061E, pk(0 if byte() < 0x80 else byte())))
    w.append((None, 0x01D1, pb(byte())))
    w.append((None, 0x05BD, pb(byte())))
    w.append((None, 0x0FFB, pk(0 if byte() < 0xA0 else byte() % 9)))
    w.append((None, 0xC936, pb(byte() % 4)))
    w.append((None, 0xC38E, pk(0xC0)))
    for i in range(23):
        si = 0x0AB3 + 0x1C * i
        w.append((None, si + 0xC, pb(0xC5 if byte() < 0x20 else 0xC0 + byte() % 5)))
        w.append((None, si + 0xF, pk(word() & 0x7FFF)))
        w.append((None, si + 0x12, pk(0x41E * (i % 8))))
    for k in range(8):
        w.append((CSEG, 0x41E * k + 0x2D, pb(byte())))
    k = byte()
    w.append((None, 0x680F, pb(0 if k < 0x80 else 1 if k < 0xC0 else byte() % 4)))
    w.append((None, 0x67EA, pk(0x3F0C if byte() < 0x80 else 0x3F3B) + pk(LOAD_SEG)))
    w.append((None, 0x05B1, pk(0x40 if byte() < 0x18 else 0)))
    w.append((None, 0xCCBC, pk(byte() % 8)))
    day = holidays[byte() % len(holidays)] if byte() < 0x60 else word() % 360
    w.append((None, 0x05CC, struct.pack('<I', (byte() % 30) * 518400 + day * 1440 + word() % 1440)))
    w.append((None, 0x05E4, pb(byte() & 7)))
    w.append((None, 0xCB5B, pb(2 if byte() < 0x80 else byte() % 4)))
    w.append((None, 0x05C0, pk(0 if byte() < 0x80 else byte() % 8)))
    w.append((None, 0x01FD, pk(byte() % 12)))
    w.append((None, 0x0FF7, pk(0 if byte() < 0xC0 else 1)))
    w.append((None, 0x0FF9, pk(0 if byte() < 0xC0 else 1)))
    w.append((None, 0x1013, pb(0 if byte() < 0xE0 else 1)))
    w.append((None, 0xC398, pk(word() & 0x0004)))
    w.append((None, 0xCCFE, pk(word() % 0x400)))
    w.append((None, 0x0604, bytes(byte() % 8 for _ in range(4))))
    w.append((None, 0x392D, pb(byte() % 4)))
    w.append((None, 0x392E, bytes(byte() % 8 for _ in range(4))))
    w.append((None, 0x05C8, pk(word() & 0x0100)))
    w.append((None, 0x191F, pb(1 if byte() < 0x20 else 0)))
    t = word()
    w.append((None, 0x0151, pk(t)))
    w.append((None, 0x884A, pk(t - (0x5A if byte() < 0x80 else byte() % 0x5A))))
    w.append((None, 0x194B, pk(0 if byte() < 0x80 else 0x1044)))
    w.append((None, 0x1044, b''.join(pk(byte() % 5 - 2) for _ in range(0x10))))
    w.append((None, 0x1949, pk(byte() % 16)))
    w.append((None, 0xFFDE, pb(byte() & 1)))
    w.append((None, 0xC3D0, pk(0x1000 + word() % 0x2000)))
    w.append((None, 0xC3D4, pk(0x1000 + word() % 0x2000)))
    cb90 = word() & 0x7EFE
    w.append((None, 0xCB90, pk(cb90)))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    # Flats in the wall plane.
    for _ in range(byte() % 24):
        k = byte()
        w.append((WSEG, word() & 0x7FFE, pk((0x8000 if k < 0xC0 else 0x9000) | word() & 0x0F00 | k % 10)))
    # A level's text shown (0000:0550): its time, its square (the
    # player's half the time), the text, the riddle waiting (the text's
    # now and then), the font and the bank.
    w.append((None, 0x3C82, pb(0 if byte() < 0x60 else byte())))
    w.append((None, 0x0DBC, pk(cb90 if byte() < 0x80 else word() & 0x7EFE)))
    txt = word() if byte() < 0xC0 else 0
    w.append((None, 0x3D42, pk(txt)))
    w.append((None, 0x7C5E, pk(txt if byte() < 0x60 else word())))
    w.append((None, 0x102C, pk(word())))
    w.append((None, 0xC8FE, pb(byte())))
    return w


def unrelocated(a, mem):
    """The message's segment (061C) and the guards' hook's (67EC)
    image-relative."""
    return gen_bash.unrelocated(gen_input.unrelocated(a, mem))


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MouseArena(EXE)
    for seg, off, code in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    # The riddle's prompt leaves DS and ES at DGROUP, as 292B:05A2 sets
    # them (0000:059C then reads 3D42 there): mov cx, DGROUP; mov ds, cx;
    # mov es, cx; inc word [FFD2]; retf.
    a.mu.mem_write(a.lin(LOAD_SEG + 0x292B, 0x05A2),
                   b'\xB9' + struct.pack('<H', a.ds) + b'\x8E\xD9\x8E\xC1\xFF\x06\xD2\xFF\xCB')
    # The text's lines (0000:05B5, called with DS at the EMS frame) end
    # with DS at DGROUP (05FF): mov cx, DGROUP; mov ds, cx; inc word
    # [FFD0]; ret.
    a.mu.mem_write(a.lin(LOAD_SEG, 0x05B5), b'\xB9' + struct.pack('<H', a.ds) + b'\x8E\xD9\xFF\x06\xD0\xFF\xC3')
    a.w16(0x902E, 0)
    a.w16(0x9030, HSEG)
    a.w16(0xBA54, 0)
    a.w16(0xBA56, 0x6400)
    a.w16(0xD806, WSEG)
    a.w16(0x1042, CSEG)
    a.w16(0x91F0, 0)
    a.w16(0xCCD0, 0x7000)
    a.w16(0xC357, 0x80)
    a.w16(0xCCCE, HSEG + 0xC00)
    a.w16(0xC359, 0x80)
    holidays = []
    at = 0x33AD
    while a.r16(at) <= 0x158:
        holidays.append(a.r16(at))
        at += 2
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_481E)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, CSEG, TSEG, HSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed, holidays):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        a.motion = (0, 0)
        before = unrelocated(a, a.seg_bytes(a.ds))
        try:
            a.run_range(0, 0x481E, 0x4CE7)
            after = unrelocated(a, a.seg_bytes(a.ds))
            changed = changes(before, after)
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'frame | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
