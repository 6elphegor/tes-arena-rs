"""Emit test vectors for the piles of treasure made (src/piles.rs
`piles`, 2433:0000) from the original code.

Usage: gen_piles.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/piles.rs): making
the level's piles or one at a square (4C91), the piles in use (FF2) and
the table in EMS bank 6 (their levels, and the limit the full-level test
reads through the frame, frame:4D76), the place (5E6, D48, 191D, C398,
C39A), the level's spots (C765, the pairs at C6E5), the level (BB0A),
the climate (A47), the clicked square (12C0), the bank to put back
(12C4), the RNG seed (91F6), the rolls and the pile record left from
before (4C8D, 4BF6), DGROUP's limit (4D76), the potions' value (5CC,
5CE), the flats (C934) and what follows the taken spots (1395). EMS is
on (20h banks, bank 0 mapped) in three cases of four; with it off the
table is in the frame itself.

Nothing is stubbed: the item makers (1194:B7FA, B848, E889, E8B2, E922,
E94E) run. DGROUP's relocated words are put back to image-relative
values, as in the port.

  piles | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes, of bank 6 and of the frame as the program sees them, and of the
map plane's changes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from dos import DosArena
from gen_attack import stream

WSEG = 0x7C00
FRAME = 0xA000


def setup(seed):
    """(writes to DGROUP, bank 6's or the frame's bytes, EMS on).
    Mirrored in tests/piles.rs: every draw is unconditional or under an
    explicit if, in this order."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    making = byte() < 0xC0
    w.append((0x4C91, bytes([0xFF if making else 0])))
    ems = byte() < 0xC0
    k = byte() % 8
    if k == 0:
        count = 0
    elif k == 1:
        count = 0xC8
    elif k == 2:
        count = byte() % 0xC8
    else:
        count = byte() % 6
    w.append((0x0FF2, bytes([count])))
    w.append((0x05E6, bytes([(3, 4, 4, 2)[byte() % 4]])))
    w.append((0x0D48, bytes([byte() % 0xC])))
    w.append((0x191D, bytes([byte() & 1])))
    k = byte() % 8
    if k == 0:
        spots = byte()
    elif k == 1:
        spots = byte() % 3
    else:
        spots = 2 + byte() % 20
    w.append((0xC765, bytes([spots])))
    v = word() & ~0x200
    if byte() < 0x20:
        v |= 0x200
    w.append((0xC39A, pk(v)))
    v = word() & ~4
    if byte() < 0x50:
        v |= 4
    w.append((0xC398, pk(v)))
    if byte() < 0xE0:
        level = byte() % 8
    else:
        level = byte()
    w.append((0xBB0A, bytes([level])))
    if byte() < 0xE0:
        climate = byte() % 3
    else:
        climate = byte()
    w.append((0x0A47, bytes([climate])))
    w.append((0x12C0, pk(word())))
    if byte() < 0xC0:
        w.append((0x12C4, pk(byte() % 8)))
    else:
        w.append((0x12C4, pk(word())))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    w.append((0x91F0, pk(0)))
    rolls = bytearray(4)
    for i in range(4):
        if byte() < 0xC0:
            rolls[i] = 0
        else:
            rolls[i] = byte()
    w.append((0x4C8D, bytes(rolls)))
    w.append((0x4BF6, bytes(byte() for _ in range(0x1C))))
    if byte() < 0xE0:
        w.append((0x4D76, bytes([byte() % 8])))
    else:
        w.append((0x4D76, bytes([byte()])))
    w.append((0x05CC, pk(word())))
    w.append((0x05CE, pk(word())))
    spot = bytearray()
    for _ in range(130):
        k = byte()
        if k < 0x20:
            spot.append(0xFF)
        elif k < 0x50:
            # Columns from 80h, which the doubling wraps.
            spot.append(0x80 + byte() % 0x7F)
        else:
            spot.append(byte() % 0x40)
        spot.append(byte() % 0x40)
    w.append((0xC6E5, bytes(spot)))
    w.append((0xC934, bytes(byte() for _ in range(8))))
    w.append((0x1395, bytes(byte() for _ in range(0x40))))
    bank = bytearray(0x10000)
    for i in range(count):
        at = 0xA000 + 0x1C * i
        for j in range(0x1C):
            bank[at + j] = byte()
        if byte() < 0x80:
            bank[at + 4] = level
        else:
            bank[at + 4] = byte() % 8
    bank[0x4D76] = byte() % 8
    return w, bytes(bank), ems


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    # 2433:0298: the INT 3 before a pile's gold is mended; DOS ignores it.
    a.GAME_INT3 = a.GAME_INT3 | {(0x2433, 0x0299)}
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    a.enable_ems(FRAME)
    snap = a.snapshot()
    rng = random.Random(0x1994_2433)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        a.w16(0xD806, WSEG)
        writes, bank, ems = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        if ems:
            a.ems['banks'][6] = bank
        else:
            mu.mem_write(a.lin(a.ds, 0x8FA6), b'\x00')
            mu.mem_write(a.lin(FRAME, 0), bank)
        before = (a.seg_bytes(a.ds), a.seg_bytes(WSEG))
        try:
            a.call_far(0x2433, 0x0000, es=a.ds)
            digests = [fnv(changes(before[0], a.seg_bytes(a.ds))), fnv(a.ems_bank(6)), fnv(a.seg_bytes(FRAME)),
                       fnv(changes(before[1], a.seg_bytes(WSEG)))]
            res = ' '.join(f'{d:x}' for d in digests)
        except Fault:
            res = 'stuck'
        except AssertionError:
            res = 'stuck'
        out.write(f'piles | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
