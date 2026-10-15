"""Emit test vectors for the status box's conditions (src/status.rs
`status_conditions`, 2686:005E).

Usage: gen_conditions.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/conditions.rs) the
player's condition flags (+40Dh, each of bits 0..0Fh set at random), its
spell bits (+41Ah, +41Ch: 0 half the time), its health and maximum
(+59h, +5Bh: a third of the cases aimed at health = maximum / 10 and its
neighbours), 5B1 (bit 10h at random), D39, the line count 12B2, and the
11h effect slots from 660h (3Ch bytes each, the 11h-th past the table, as
2686:0158 walks it): bytes at random, then +3Bh FFh (free) a quarter of
the time, the far target +28h the player's (DS:01A4h) half the time (a
quarter of those, and all of them in a quarter of the cases, with
another segment), the
kind +3Ah 4, 0Ah..18h or any byte, and +6..+0Dh zero or not. The text
starts at 941Eh with a few letters; DI is its NUL.

  conditions | seed | digest

The digest is FNV-1a of the changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, changes, fnv, mine
from gen_attack import stream

PLAYER = 0x01A4
SLOTS = 0x0660


def setup(seed):
    """(writes, slots on the player (index list), di). Mirrored in
    tests/conditions.rs. A slot on the player gets its target's segment
    written by the caller (DGROUP as each side sees it)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    flags = 0
    for bit in range(16):
        if byte() < 0x30:
            flags |= 1 << bit
    w.append((PLAYER + 0x40D, struct.pack('<H', flags)))
    w.append((PLAYER + 0x41A, struct.pack('<H', word() if byte() & 1 else 0)))
    w.append((PLAYER + 0x41C, struct.pack('<H', word() if byte() & 1 else 0)))
    mx = word()
    k = byte() % 3
    hp = word() if k else (mx // 10 + (byte() % 3) - 1) & 0xFFFF
    w.append((PLAYER + 0x59, struct.pack('<H', hp)))
    w.append((PLAYER + 0x5B, struct.pack('<H', mx)))
    w.append((0x05B1, struct.pack('<H', word() & ~0x10 | (0x10 if byte() & 1 else 0))))
    w.append((0x0D39, bytes([byte()])))
    w.append((0x12B2, struct.pack('<H', word())))
    on = []
    # A quarter of the cases give no slot the player's segment.
    no_seg = byte() < 0x40
    for i in range(0x11):
        rec = bytearray(byte() for _ in range(0x3C))
        if byte() < 0x40:
            rec[0x3B] = 0xFF
        elif rec[0x3B] == 0xFF:
            rec[0x3B] = 0
        if byte() & 1:
            rec[0x28:0x2A] = struct.pack('<H', PLAYER)
            # A quarter keep a random segment: the player's offset alone,
            # which 2686:0270 compares and 0158/01C3 don't.
            if byte() >= 0x40 and not no_seg:
                on.append(i)
        kind = byte()
        rec[0x3A] = (4, 0x0A + kind % 15, kind)[byte() % 3]
        for j in range(8):
            if byte() < 0xA0:
                rec[6 + j] = 0
        w.append((SLOTS + 0x3C * i, bytes(rec)))
    n = byte() % 8
    w.append((0x941E, bytes(0x61 + byte() % 26 for _ in range(n)) + b'\0'))
    return w, on, 0x941E + n


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_005E)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, on, di = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        for i in on:
            a.w16(SLOTS + 0x3C * i + 0x2A, a.ds)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x2686, 0x005E, di=di)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'conditions | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
