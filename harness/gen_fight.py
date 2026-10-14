"""Emit test vectors for a fight starting (src/encounter.rs
`fight_prepare`, 25CA:0000, then `place_creatures`, 25CA:006A) from the
original code.

Usage: gen_fight.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/fight.rs) the
creature slots' flags, the creature records' speed and flyer bit, the
actors' squares, the walls and floors around the player, the player's
square, the fight's state (0FF9, DBE, FF3), the colour remap, the
dungeon flats and the random seed; then runs 25CA:0000 with AL and
25CA:006A with AX = the count.

Stubbed (counting calls at DGROUP FFE0..FFE4, as the port's test hooks
do): the creatures' graphics 0000:1AF9 and 1A5C, and the music
23BA:01C7.

  fight | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes, the wall plane and the remap segment.
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

WSEG, FSEG, CSEG, RSEG, FRAME = 0x6800, 0x7000, 0x7800, 0x6000, 0x8800


def setup(seed):
    """(segment or None, offset, bytes) writes and (al, count). Mirrored
    in tests/fight.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    px, py = 0x1000 + word() % 0x2000, 0x1000 + word() % 0x2000
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    w.append((None, 0xCB90, pk((py >> 7 << 8) + (px >> 7 << 1))))
    for i in range(8):
        si = 0x0C57 + 0x1C * i
        w.append((None, si, bytes(byte() for _ in range(0x1C))))
        w.append((None, si + 0xF, pk(word() & 0xBEFF | (0x4000 if byte() < 0xA0 else 0))))
        rec = 0x41E * i
        w.append((CSEG, rec + 0x2D, pb(byte())))
        w.append((CSEG, rec + 0x40D, pk(word() & 0x1000)))
    for i in range(23):
        k = byte()
        sq = (((py >> 7) + byte() % 9 - 4) << 8) + (((px >> 7) + byte() % 9 - 4) << 1) if k < 0x40 else 0xFFFF
        w.append((None, 0x1951 + 2 * i, pk(sq)))
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            sq = (((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)
            k = byte()
            w.append((WSEG, sq & 0xFFFF, pk(0 if k < 0xB0 else 0x8000 | byte() if k < 0xC8 else word() & 0x7FFF)))
            k = byte()
            w.append((FSEG, sq & 0xFFFF, pk(FRAME if k < 0xE0 else FRAME + 0xC00 + byte())))
    w.append((None, 0x0FF9, pk(0 if byte() < 0x80 else byte() % 10)))
    w.append((None, 0x0DBE, pk(word())))
    w.append((None, 0x0FF3, pk(word())))
    w.append((None, 0x0F74, pk(0 if byte() < 0xC0 else word())))
    w.append((None, 0x100D, pb(byte())))
    w.append((None, 0x05E6, pb(4 if byte() < 0x80 else byte() % 6)))
    w.append((None, 0x191D, pb(byte() & 1)))
    w.append((None, 0xC943, pb(byte())))
    w.append((None, 0xC987, bytes(byte() for _ in range(8))))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    k = byte()
    return w, (byte() if k < 0xC0 else 0), (1 + byte() % 6 if byte() < 0xF0 else byte() % 12)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    for o, v in [(0xD806, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0xCCC2, RSEG), (0x9030, FRAME),
                 (0xCCCE, FRAME + 0xC00), (0x91F0, 0), (0x05C8, 0)]:
        a.w16(o, v)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_25CA)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, FSEG, CSEG, RSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, al, count = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x25CA, 0x0000, eax=al)
            a.call_far(0x25CA, 0x006A, eax=count)
            after = a.seg_bytes(a.ds)
            changed = changes(before, after)
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x} {fnv(a.seg_bytes(RSEG)[:0x1000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'fight | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
