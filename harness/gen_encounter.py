"""Emit test vectors for random encounters (src/encounter.rs
`random_encounter`, 1194:887A) from the original code.

Usage: gen_encounter.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/encounter.rs) the
hour and date (some on the special nights), the area (5E6, 1013, D48,
191D, the terrain A50), the player's square (some in a building), the
fight and scene flags, the player's level, the creature slots, the
quests (some waiting for a creature), the walls around the player and
the random seed; then runs 1194:887A with the game's heap set up and
DOS handle 5 taken (GLOBAL.BSA's in the game).

Stubbed as gen_guards.py: the fight's graphics and music count calls at
DGROUP FFE0..FFE4; Jagar's movie and text (1194:D45D) at FFE6 (the
text, AX, kept at FFEC), the
view's save and restore around it (0000:A611, A6B5) at FFE8, FFEA.
The people's graphics list (2686:0423) runs.

  encounter | seed | result

Results as gen_guards.py.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG
from emu import changes
from emu import mine
from dos import DosArena
from gen_attack import fnv, stream
from gen_frame import counter
from gen_guards import WSEG, FSEG, CSEG, FRAME


def setup(seed):
    """(segment or None, offset, bytes) writes. Mirrored in
    tests/encounter.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    px, py = 0x1000 + word() % 0x2000, 0x1000 + word() % 0x2000
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    sq = (py >> 7 << 8) + (px >> 7 << 1)
    w.append((None, 0xCB90, pk(sq)))
    w.append((None, 0x05D6, pk(byte() % 24)))
    k = byte()
    day = (0xF3 if byte() < 0x80 else 0x11B) if k < 0x40 else word() % 360
    w.append((None, 0x05CC, struct.pack('<I', (byte() % 30) * 518400 + day * 1440 + word() % 1440)))
    w.append((None, 0x05E6, pb(byte() % 6)))
    w.append((None, 0x1013, pb(1 if byte() < 0x30 else 0)))
    w.append((None, 0x0D48, pb(byte() % 12)))
    w.append((None, 0x191D, pb(byte() & 1)))
    w.append((None, 0x0A50, pb(byte() % 8)))
    w.append((None, 0x05CA, pk(0x200 if byte() < 0x10 else 0x40 if byte() < 0x40 else 0)))
    w.append((None, 0x0FF7, pk(1 if byte() < 0x10 else 0)))
    w.append((None, 0x0FFB, pk(1 if byte() < 0x10 else 0)))
    w.append((None, 0x0FF9, pk(0 if byte() < 0xC0 else 1 + byte() % 3)))
    w.append((None, 0x01AA, pb(byte() % 30)))
    for i in range(8):
        si = 0x0C57 + 0x1C * i
        w.append((None, si + 0xF, pk(word() & 0x3FFF | (0x4000 if byte() < 0xC0 else 0))))
    for q in range(4):
        e = 0x0E6E + 0x34 * q
        k = byte()
        stage = 6 if k < 0x30 else 4 if k < 0x60 else byte() % 8
        w.append((None, e + 4, pk(0x40 | stage << 9)))
        w.append((None, e + 0x2F, pb(0x18 + byte() % 8 if byte() < 0x80 else byte() % 0x18)))
        w.append((None, e + 0x30, pb(0x18 + byte() % 8 if byte() < 0x80 else byte() % 0x18)))
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            at = (((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)
            w.append((WSEG, at & 0xFFFF, pk(0 if byte() < 0xC0 else word() & 0x7FFF)))
    w.append((WSEG, sq, pk(0 if byte() < 0xC0 else 0x0102 * (1 + byte() % 0x40))))
    w.append((CSEG, 4, pb(byte() % 0x30)))
    w.append((CSEG, 5, pb(byte() % 8)))
    w.append((CSEG, 0x40D, pk(word() & 0x1000)))
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    # The main quest's encounters (5C8 bit 8000h): Jagar's words first
    # when F77 is set (1194:D91C).
    if byte() < 0x40:
        w.append((None, 0x05C8, pk(0x8000)))
        w.append((None, 0x0F77, pb(0 if byte() < 0x40 else byte() % 9)))
        w.append((None, 0xA902, pk(word())))
    return w


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True)), (0x1194, 0xD45D, bytes.fromhex('a3ecffff06e6ffc3')),
                           (0, 0xA611, counter(0xFFE8, True)), (0, 0xA6B5, counter(0xFFEA, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    for o, v in [(0xD806, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0x9030, FRAME), (0xCCCE, FRAME + 0xC00),
                 (0x91F0, 0), (0x05C8, 0), (0xC38E, 0xC0), (0xC357, 0x80), (0xC359, 0x80)]:
        a.w16(o, v)
    a.init_heap()
    a.handles[5] = [b'', 0]
    base_ds = a.seg_bytes(a.ds)
    base_heap = bytes(a.mu.mem_read(0xA0000, 0x50000))
    zero = bytes(0x10000)
    rng = random.Random(0x1994_887A)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(0xA0000, base_heap)
        for seg in (WSEG, FSEG, CSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x887A)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            changed = changes(before, after)
            res = (f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} '
                   f'{fnv(bytes(a.mu.mem_read(0xA0000, 0x50000))):x}')
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'encounter | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
