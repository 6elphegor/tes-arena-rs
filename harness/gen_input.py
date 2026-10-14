"""Emit test vectors for the player's controls (src/controls.rs
`player_input`, 2826:0002) from the original code.

Usage: gen_input.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/input.rs) the mouse
(cursor, buttons, last buttons), the keys the controls read, the
player's position, heading and height, the walls and floors around the
player, the handlers, speeds and jump state, a spell waiting for a
target, the flags the controls test and the cursor image list (seg
[1002]), and runs N frames of 2826:0002.

Stubbed, as the port's ControlHooks or the renderer's: the look
2826:0C15 and the spell counters 2826:0BB2 (RET); the interface bar
2542:00BA, the wilderness 217B:0265, the wall triggers 0000:43B1, the
cast at a target 29CF:0000, the message box 1194:42EA/59C8, the stat
bars 2901:00A8, the cursor drawing 0000:541E and 77EC, the view refresh
0000:DA8A (RETF) and DA8E (RET), the sound 23BA:002A (RETF), and the
mouse driver ([B028] at a RET: the case's cursor and buttons are its
report). The floor under the cursor (2826:0B52) is read from FFF0.

  input | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes and the wall plane (32 KB).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes
from emu import mine
from gen_attack import fnv, stream

WSEG, FSEG, FRAME, CURSEG = 0x6800, 0x7000, 0x8800, 0x7800
HANDLERS = [0x0750, 0x055A, 0x0768, 0x06A0, 0x077E, 0x06FA, 0x05FE, 0x05A9, 0x0651]
KEYS = [0x1D, 0x24, 0x2A, 0x36, 0x48, 0x4B, 0x4D, 0x50]


def setup(s):
    """(segment or None for DGROUP, offset, bytes) writes and the frame
    count, from the byte stream `s`. Mirrored in tests/common/controls.rs."""
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    # The mouse: the cursor (mostly in the view), buttons now and before.
    k = byte()
    x = word() % 0x140 if k < 0xE0 else word()
    k = byte()
    y = byte() % 0x93 if k < 0xC0 else byte() % 0xC8 if k < 0xF0 else word()
    w.append((None, 0x8F92, pk(x)))
    w.append((None, 0x8F94, pk(y)))
    w.append((None, 0xAFF9, pb(byte() & 3)))
    w.append((None, 0x7926, pb(byte() & 3)))
    for scan in KEYS:
        w.append((None, 0xACB5 + scan, pb(1 if byte() < 0x40 else 0)))
    w.append((None, 0xA91A, pk(0x93 if byte() < 0xC0 else 0xC7)))
    # The player.
    px, py = 0x200 + word() % 0x1C00, 0x200 + word() % 0x1C00
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    w.append((None, 0xC3DA, pk(word() & 0x1FF)))
    k = byte()
    w.append((None, 0xC3D2, pk(0x3C if k < 0x80 else 0x3B if k < 0x90 else 0x3C + byte() % 0x40 if k < 0xC0 else word())))
    sq = (py >> 7 << 8) + (px >> 7 << 1)
    w.append((None, 0xCB90, pk(sq)))
    w.append((None, 0xCB92, pk(word())))
    w.append((None, 0x01A8, pb(7 if byte() < 0x40 else byte() % 8)))
    w.append((None, 0x01A9, pb(0xC9 if byte() < 0x20 else byte())))
    w.append((None, 0x01CD, pb(byte())))
    w.append((None, 0x020A, pk(word())))
    # The squares around the player.
    for dy in range(-2, 3):
        for dx in range(-2, 3):
            at = sq + dy * 0x100 + dx * 2
            k = byte()
            wall = 0 if k < 0xA0 else (1 + byte() % 0x3F) * 0x101 if k < 0xE0 else word() & 0x7FFF
            w.append((WSEG, at, pk(wall)))
            k = byte()
            floor = FRAME + 0xC00 if k < 0x40 else word()
            w.append((FSEG, at, pk(floor)))
    w.append((None, 0xCCCE, pk(FRAME + 0xC00 if byte() < 0xC0 else word())))
    w.append((None, 0xD80C, pk(0x96 if byte() < 0xC0 else word() & 0x1FF)))
    w.append((None, 0xC942, pb(byte())))
    # Handlers, speeds, jumping.
    w.append((None, 0x7A54, pk(HANDLERS[byte() % 9])))
    w.append((None, 0x7A56, pk(HANDLERS[byte() % 9])))
    k = byte()
    w.append((None, 0x7A46, pk(0 if k < 0x60 else 0x100 if k < 0xA0 else word() & 0x1FF)))
    k = byte()
    w.append((None, 0xCCBA, pk(0 if k < 0x80 else byte() if k < 0xF0 else word())))
    w.append((None, 0xCCBE, pk(0x28 if byte() < 0xC0 else byte())))
    w.append((None, 0xC392, pk(0x10 if byte() < 0xC0 else byte())))
    k = byte()
    w.append((None, 0x05E3, pb(0 if k < 0xA0 else 1 + byte() % 10 if k < 0xF8 else byte())))
    w.append((None, 0xE39F, pb(0 if byte() < 0xC0 else byte() % 8)))
    w.append((None, 0x7A6C, pk(word())))
    w.append((None, 0x79A9, pb(byte() & 1)))
    # A spell waiting for a target, the message.
    for o in (0x0DC0, 0x0DC2, 0x0DC4, 0x0DC8, 0x0620):
        k = byte()
        v = 0 if k < 0xC0 else 0x7D0 if k < 0xD0 else word()
        # The spell cast at a target is reached through a far pointer:
        # at 1044, to a zeroed segment.
        w.append((None, o, pk(0x1044 if o == 0x0620 and v else v)))
    w.append((None, 0x1044, pk(0) + pk(0x6000)))
    w.append((None, 0x0F84, pk(0x8000 if byte() < 0x40 else word())))
    w.append((None, 0x7DBA, pk(word())))
    w.append((None, 0x061E, pk(0xFFFE if byte() < 0x40 else word())))
    k = byte()
    w.append((None, 0x43CA, pk(0 if k < 0xE0 else word())))
    w.append((None, 0x43CC, pk(word())))
    # Flags.
    w.append((None, 0xC39A, pk(word() & 0xC293)))
    w.append((None, 0xC398, pk(word() & 0x0985)))
    w.append((None, 0x05B1, pk(word() & (0x40 if byte() < 0x20 else 0))))
    w.append((None, 0x05E0, pk(word() & 0x2450)))
    w.append((None, 0x05CA, pk(word() & 0x9013)))
    w.append((None, 0xCCBC, pk(word())))
    # Cursor images.
    w.append((CURSEG, 0, bytes(byte() for _ in range(0x100))))
    return w, 1 + byte() % 6


# 2826:0B52 (the floor under the cursor) reads the square from DGROUP FFF0
# (FFFFh none): mov si,[FFF0]; cmp si,-1; jne +2; stc; ret; clc; ret.
FLOOR_PICK = bytes.fromhex('8b36f0ff83feff7502f9c3f8c3')


def prepare(a):
    """The stubs and segments both generators share."""
    for seg, off in [(0x2826, 0x0C15), (0x2826, 0x0BB2), (0, 0xDA8E)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xc3')
    for seg, off in [(0x2542, 0x00BA), (0x217B, 0x0265), (0x2901, 0x00A8), (0, 0x541E), (0, 0x77EC),
                     (0, 0xDA8A), (0x23BA, 0x002A), (0, 0x43B1), (0x29CF, 0x0000), (0x1194, 0x42EA),
                     (0x1194, 0x59C8)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    a.mu.mem_write(a.lin(LOAD_SEG + 0x2826, 0x0B52), FLOOR_PICK)
    a.w16(0xB028, 0x8FC2)
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    a.w16(0x9030, FRAME)
    a.w16(0x1002, CURSEG)
    a.w16(0x91F0, 0)
    a.mu.mem_write(a.lin(a.ds, 0xC917), b'\xff' * 16)
    a.mu.mem_write(a.lin(a.ds, 0xC932), b'\xff\xff')


def unrelocated(a, mem):
    """The message's segment (061C) image-relative: DGROUP or the data
    segment 3276."""
    mem = bytearray(mem)
    v = struct.unpack_from('<H', mem, 0x061C)[0]
    if v == a.ds:
        struct.pack_into('<H', mem, 0x061C, DGROUP)
    elif v == LOAD_SEG + 0x3276:
        struct.pack_into('<H', mem, 0x061C, 0x3276)
    return bytes(mem)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    prepare(a)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = __import__('random').Random(0x1994_2826)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, FSEG, CURSEG, 0x6000):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, frames = setup(stream(seed))
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = unrelocated(a, a.seg_bytes(a.ds))
        try:
            for _ in range(frames):
                a.call_far(0x2826, 0x0002)
            after = unrelocated(a, a.seg_bytes(a.ds))
            changed = changes(before, after)
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'input | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
