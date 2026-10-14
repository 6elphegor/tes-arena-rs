"""Emit test vectors for the rebuild after loading a game (src/load.rs
`save_rebuild`, 25FD:03F9..051E) from the original code.

Usage: gen_load.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

SAVE_DIR holds a saved game whose slot 1 has SPELLSG.01, SPELLS.01 and
CITYDATA.01 (the user's save); slot 65's come from GAME_DIR. Vectors hold
digests only.

A case fills from a seed (`setup`, mirrored in tests/load.rs) the area
(5E6), 5C8 (bit 4 the title screen's way), 5E4, 5E0, C398, C39A, the
light table's segment (CCC2), CB5C, the province (A46), the location
(A4C), the slot (1A6C: 1 or 65), the working slot (1A80), FF9, 6636, the
first creature record (its type, +40Dh, the rest), the other records'
+40Dh, the eight creature slots' flags and the effects' slots' state and
target, then runs 25FD:03F9 up to its RETF at 051E.

Counted in DGROUP (far stubs): 0000:1AF9 (FFE0), 1A5C (FFE2), 23BA:01C7
(FFE4), 0000:E5C9 (FFE6), 2294:0006 (FFE8), 0000:AAA2 (FFEA), 25FD:056F
(FFEC), 1194:3AE8 (FFEE), 292B:09AC (FFF0), 2F9F:001F (FFF2), 0000:3503
(FFF4). Everything else runs: 1194:867E with 8ED4, 8B12, 25CA:0170,
2686:0423 and 25CA:0000; 2EBD:054C; 29CF:0691 and 1194:2E14 on the
files; 217B:0004; 0000:0B15 and 0B4F; 25FD:0539, 051F, 0844.

  load | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes (the effects' slots' DGROUP segment, A90A, the level hooks'
segment 0000 and BA7C unrelocated), the
creature records (8 x 41Eh), the spells (3276:0000..3520) and the 80h
bytes at the far pointer 1020.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter

CSEG = 0x7000
EXTRA = (0x8800, 0x0010)
PLAYER = 0x01A4
SLOTS, SLOT_SIZE = 0x0660, 0x3C


def setup(seed):
    """Writes (segment None: DGROUP). Mirrored in tests/load.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    w.append((None, 0x05E6, pb(byte() % 5 if byte() < 0xF0 else byte())))
    w.append((None, 0x05C8, pk(word() & ~4 | (4 if byte() < 0x40 else 0))))
    w.append((None, 0x05E4, pb(byte())))
    w.append((None, 0x05E0, pk(word())))
    w.append((None, 0xC398, pk(word() & 0x7FFB | (4 if byte() < 0x80 else 0) | (0x8000 if byte() < 0x40 else 0))))
    w.append((None, 0xC39A, pk(word() & ~0x200 | (0x200 if byte() < 0x40 else 0))))
    w.append((None, 0xCCC2, pk(0 if byte() < 0x80 else word())))
    w.append((None, 0xCB5C, pb(byte())))
    w.append((None, 0x0A46, pb(byte() % 9 if byte() < 0xE0 else byte())))
    w.append((None, 0x0A4C, pb(byte())))
    w.append((None, 0x1A6C, pb((1, 65)[byte() & 1])))
    w.append((None, 0x1A80, pb(byte())))
    w.append((None, 0x0FF9, pk(word())))
    w.append((None, 0x6636, pb(byte())))
    rec =bytearray(byte() for _ in range(0x41E))
    rec[4] = 0x2A if byte() < 0x20 else 1 + byte() % 0x29
    w.append((CSEG, 0, bytes(rec)))
    for k in range(1, 8):
        w.append((CSEG, 0x41E * k + 0x40D, pk(word())))
    for i in range(8):
        w.append((None, 0x0C57 + 0x1C * i + 0xF, pk(word() & 0x3FFF | (0x4000 if byte() < 0xA0 else 0))))
    for i in range(16):
        sl = SLOTS + SLOT_SIZE * i
        w.append((None, sl + 0x3B, pb(0xFF if byte() < 0x80 else byte())))
        w.append((None, sl + 0x28, pk(PLAYER if byte() < 0x80 else word())))
    return w


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME, SAVE)
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True)), (0, 0xE5C9, counter(0xFFE6, True)),
                           (0x2294, 0x0006, counter(0xFFE8, True)), (0, 0xAAA2, counter(0xFFEA, True)),
                           (0x25FD, 0x056F, counter(0xFFEC, True)), (0x1194, 0x3AE8, counter(0xFFEE, True)),
                           (0x292B, 0x09AC, counter(0xFFF0, True)), (0x2F9F, 0x001F, counter(0xFFF2, True)),
                           (0, 0x3503, counter(0xFFF4, True))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.w16(0x1042, CSEG)
    a.w16(0x1020, EXTRA[1])
    a.w16(0x1022, EXTRA[0])
    base = a.snapshot()
    zero = bytes(0x41E * 8)
    rng = random.Random(0x1994_03F9)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mu.mem_write(a.lin(CSEG, 0), zero)
        a.mu.mem_write(a.lin(EXTRA[0], EXTRA[1]), bytes(0x80))
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.run_range(0x25FD, 0x03F9, 0x051E)
            for i in range(16):
                at = SLOTS + SLOT_SIZE * i + 0x2A
                if a.r16(at) == a.ds:
                    a.w16(at, a.ds - LOAD_SEG)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            # Segment 0000 as the level hooks (0000:0B15, 0B4F) and the
            # way back (25FD:053A) write it: relocated.
            for at in (0xCD34, 0xCD50, 0xCD54, 0xBA7C):
                if a.r16(at) == LOAD_SEG:
                    a.w16(at, 0)
            after = a.seg_bytes(a.ds)
            parts = [changes(before, after), a.mu.mem_read(a.lin(CSEG, 0), 0x41E * 8),
                     a.mu.mem_read(a.lin(LOAD_SEG + 0x3276, 0), 0x3520),
                     a.mu.mem_read(a.lin(EXTRA[0], EXTRA[1]), 0x80)]
            res = ' '.join(f'{fnv(p):x}' for p in parts)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'load | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
