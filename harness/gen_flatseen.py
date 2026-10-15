"""Emit test vectors for a creature's flat coming alive (src/encounter.rs
`flat_creature`, 25CA:0271, the hook CD56 the drawing calls for a
marked flat) from the original code.

Usage: gen_flatseen.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/flatseen.rs) the
player's place, the area and the fight and scene flags, the player's
level, the creature slots, the walls around the player, the 40h flat
numbers at C954, a flat's square within seven of the player (its word in
the walls plane, its number mostly one of the list's), the level's
trigger list (often with the flat's square) and the INF's texts (`+`
items and plain texts, EMS off) and the random seed; then runs 25CA:0271
with AX, BX the square's centre, CL the flat's number (mostly its word's
low byte) and ES:DI its word, the game's heap set up and DOS handle 5
taken.

Stubbed as gen_encounter.py: the fight's graphics and music count calls
at DGROUP FFE0..FFE4.

  flatseen | seed | result

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

TSEG = 0x8000


def setup(seed):
    """(writes as gen_encounter's, text bytes at TOFF, TOFF, regs).
    Mirrored in tests/flatseen.rs; every draw is made in order."""
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
    w.append((None, 0x05E6, pb(byte() % 6)))
    w.append((None, 0x05CA, pk(word() & 0x0640)))
    w.append((None, 0x05C8, pk(0x1000 if byte() < 0x40 else 0)))
    w.append((None, 0x0FF7, pk(1 if byte() < 0x20 else 0)))
    w.append((None, 0x0FF9, pk(0 if byte() < 0xC0 else 1 + byte() % 3)))
    w.append((None, 0x01AA, pb(byte() % 30)))
    for i in range(8):
        si = 0x0C57 + 0x1C * i
        w.append((None, si + 0xF, pk(word() & 0x3FFF | (0x4000 if byte() < 0xC0 else 0))))
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            at = (((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)
            w.append((WSEG, at & 0xFFFF, pk(0 if byte() < 0xC0 else word() & 0x7FFF)))
    w.append((CSEG, 4, pb(byte() % 0x30)))
    w.append((CSEG, 5, pb(byte() % 8)))
    w.append((CSEG, 0x40D, pk(word() & 0x1000)))
    # The monsters' flats (kinds 1..17h; the rest of the list unused).
    flats = bytes(1 + byte() % 0x7F for _ in range(0x17)) + bytes(0x29)
    w.append((None, 0xC954, flats))
    col = (px >> 7) + byte() % 15 - 7
    row = (py >> 7) + byte() % 15 - 7
    x, y = col << 7 | 0x40, row << 7 | 0x40
    di = ((row << 8) + (col << 1)) & 0xFFFF
    number = flats[byte() % 0x17] if byte() < 0xA0 else byte() | 0x80
    w.append((WSEG, di, pk(0x8200 | number)))
    cl = byte() | 0x80 if byte() < 0x20 else number
    n = byte() % 6
    w.append((None, 0xC866, pb(n)))
    for i in range(n):
        if byte() < 0x80:
            c, r = col & 0xFF, row & 0xFF
        else:
            c, r = byte() % 0x40, byte() % 0x40
        a = 0xFF if byte() < 0x20 else byte() % 3
        b = 0xFF if byte() < 0x20 else byte()
        w.append((None, 0xC766 + 4 * i, bytes([c, r, a, b])))
    for i in range(8):
        w.append((None, 0x0622 + 6 * i, pk(0xFFFF if byte() < 0x80 else word())))
    toff = word() & 0x0FFF
    w.append((None, 0xCB88, pk(toff)))
    t = b''
    for i in range(3):
        if byte() < 0xA0:
            v = 0x7B if byte() < 0x40 else byte()
            t += b'+' + str(v).encode() + b'\rITEM'
        else:
            t += b'TEXT\rL'
        t += b'\0'
    w.append((None, 0x91F6, bytes(byte() for _ in range(4))))
    return w, t, toff, dict(eax=x, ebx=y, ecx=cl, es=WSEG, edi=di)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    for o, v in [(0xD806, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0x9030, FRAME), (0xCCCE, FRAME + 0xC00),
                 (0x91F0, 0), (0x05C8, 0), (0xC38E, 0xC0), (0xC357, 0x80), (0xC359, 0x80), (0xCB8A, TSEG)]:
        a.w16(o, v)
    a.mu.mem_write(a.lin(a.ds, 0x8FA6), b'\0')
    a.init_heap()
    a.handles[5] = [b'', 0]
    base_ds = a.seg_bytes(a.ds)
    base_heap = bytes(a.mu.mem_read(0xA0000, 0x50000))
    zero = bytes(0x10000)
    rng = random.Random(0x1994_0271)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        w, t, toff, regs = setup(seed)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(0xA0000, base_heap)
        for seg in (WSEG, FSEG, CSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        a.mu.mem_write(a.lin(TSEG, 0), zero[:0x2000])
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        a.mu.mem_write(a.lin(TSEG, toff), t)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x25CA, 0x0271, **regs)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            res = (f'{fnv(changes(before, after)):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} '
                   f'{fnv(a.seg_bytes(WSEG)):x} {fnv(bytes(a.mu.mem_read(0xA0000, 0x50000))):x}')
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'flatseen | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
