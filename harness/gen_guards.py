"""Emit test vectors for the guards arriving (src/encounter.rs
`guards_arrive`, 1194:33C0) from the original code.

Usage: gen_guards.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/guards.rs) the
province (A46 race, A47), the people and creature actor slots, the
walls and floors around the player and the random seed, then runs
1194:33C0 with the game's heap set up (A000..F000).

Stubbed (counting calls at DGROUP FFE0..FFE4, as the port's test hooks
do): the graphics 0000:1AF9 and 1A5C, the music 23BA:01C7. The people's
graphics list (2686:0423) runs. The guards' names come
from NAMECHNK.DAT; the name buffer's segment (A90A) is compared
image-relative.

  guards | seed | result

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes, the creature segment (32 KB) and the heap (A000..F000).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG
from dos import DosArena
from gen_attack import fnv, stream
from gen_frame import counter

WSEG, FSEG, CSEG, FRAME = 0x6800, 0x7000, 0x7800, 0x8800


def setup(seed):
    """(segment or None, offset, bytes) writes. Mirrored in tests/guards.rs."""
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
    w.append((None, 0x0A46, pb(byte() % 8)))
    w.append((None, 0x0A47, pb(byte() % 9)))
    w.append((None, 0xC936, pb(byte() % 4)))
    for i in range(23):
        si = 0x0AB3 + 0x1C * i
        w.append((None, si + 0xC, pb(0xC0 + byte() % 6)))
        w.append((None, si + 0xF, pk(word() & 0x3FFF | (0x4000 if i >= 15 and byte() < 0xC0 else 0))))
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            sq = (((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)
            w.append((WSEG, sq & 0xFFFF, pk(0 if byte() < 0xC0 else word() & 0x7FFF)))
    w.append((None, 0x0FF9, pk(0)))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    return w


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    for o, v in [(0xD806, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0x9030, FRAME), (0xCCCE, FRAME + 0xC00),
                 (0x91F0, 0), (0x05C8, 0), (0xC38E, 0xC0), (0xC357, 0x80), (0xC359, 0x80)]:
        a.w16(o, v)
    a.init_heap()
    # GLOBAL.BSA holds handle 5 in the game (the port assumes it): a loose
    # file opens as 6.
    a.handles[5] = [b'', 0]
    base_ds = a.seg_bytes(a.ds)
    base_heap = bytes(a.mu.mem_read(0xA0000, 0x50000))
    zero = bytes(0x10000)
    rng = random.Random(0x1994_33C0)
    for n in range(int(os.environ.get('N', 2000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(0xA0000, base_heap)
        for seg in (WSEG, FSEG, CSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x33C0)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = (f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} '
                   f'{fnv(bytes(a.mu.mem_read(0xA0000, 0x50000))):x}')
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'guards | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
