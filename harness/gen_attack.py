"""Emit test vectors for src/attack.rs from the original code.

Usage: gen_attack.py ACD_unpacked.EXE OUT

A case fills the player (DGROUP 01A4) and a combatant in the creature
segment from a seed (`setup`, mirrored in tests/attack.rs) and runs one
blow:

  creature | seed k | result   2AA1:00D8, combatant k hits at the player
                                (as the actors' frame calls it: AX 0,
                                DGROUP 1400 the record, 1402 = 01A4)
  special | seed k ty | result  the same with `special`: a creature of
                                type ty that paralyses or infects

The sound (23BA:002A) and the message box of a broken special item
(1194:A36C) are UI and stubbed with a RETF.

The result is "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes (offset, value) and of the creature segment (32 KB at 7800h).
Words that hold DGROUP's segment (the message pointer at 061C, effect
slot targets) are compared image-relative.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP

EXE, OUT = sys.argv[1:3]
out = open(OUT, 'w', buffering=1)
CSEG, PLAYER, RSIZE = 0x7800, 0x01A4, 0x500


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


def stream(seed):
    x = seed
    while True:
        x = (x * 1103515245 + 12345) & 0xffffffff
        yield x >> 16 & 0xFF


def setup(seed, k, classes):
    """(segment or None for DGROUP, offset, bytes) writes, in order.
    Mirrored in tests/attack.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []

    def items(seg, at):
        for i in range(40):
            it = at + i * 19
            if byte() < 0x40:
                continue
            b = bytearray(byte() for _ in range(19))
            kind = byte() & 3
            ty = [byte() % 11, byte() % 18, byte() & 3, byte()][kind]
            cat = kind if kind < 3 else byte() & 3
            if byte() < 0x20:
                ty = 0x80 | ty & 0xF
            b[0] = ty
            b[4] = byte() % 20
            b[5] = b[4] + byte() % 12
            v = byte()
            health = byte() & 7 if v < 0x50 else 0xFFFF if v >= 0xF8 else word() % 1500
            b[6:8] = struct.pack('<H', health)
            b[14] = cat << 2 | 0x40 | (0x80 if byte() < 0x90 else 0) | b[14] & 0x31
            b[16] = byte() & 7 if byte() < 0xD0 else 0xFF
            b[18] = byte() & 7 if byte() < 0xC0 else 0xFF
            if byte() < 0x10:
                b[17] = 0xC1
            w.append((seg, it, bytes(b)))

    def record(seg, rec, creature):
        f = [
            (4, bytes([byte() % 0x18 if creature else byte() & 7])),
            (5, bytes([0x4D if byte() < 0x30 else classes[byte() % len(classes)] if byte() < 0xE0 else byte()])),
            (6, bytes([1 + byte() % 30])),
            (9, bytes([byte() % 12])),
            (0xA, bytes([12 + byte() % 30])),
            (0x39, struct.pack('<h', byte() % 16 - 4)),
            (0x47, struct.pack('<h', byte() - 0x80)),
            (0x49, struct.pack('<h', byte() - 0x80)),
            (0x57, struct.pack('<h', byte() % 64 - 32)),
            (0x59, struct.pack('<H', 1 + byte())),
            (0x5B, struct.pack('<H', 1 + word() % 400)),
            (0x62, bytes([byte()])),
            (0x40D, struct.pack('<H', (0x1000 if creature else 0) | word() & 0x0090)),
            (0x41C, struct.pack('<H', 0 if byte() < 0xA0 else byte())),
        ]
        for o, b in f:
            w.append((seg, rec + o, b))

    record(None, PLAYER, False)
    items(None, PLAYER + 0x6E)
    creature = byte() < 0xA0
    record(CSEG, k * RSIZE, creature)
    items(CSEG, k * RSIZE + 0x6E)
    if k != 0:
        # The "creature target" the weapon checks look at.
        w.append((CSEG, 4, bytes([byte() % 0x18])))
        w.append((CSEG, 0x40D, struct.pack('<H', 0x1000 if byte() < 0x80 else 0)))
    # Effect slots (some on the player), and what a death clears.
    w.append((None, 0x0660, bytes(byte() for _ in range(0x3C0))))
    for i in range(16):
        t = byte()
        at = 0x0660 + i * 0x3C
        if t < 0x80:
            w.append((None, at + 0x3B, b'\xff'))
        elif t < 0xC0:
            w.append((None, at + 0x28, struct.pack('<HH', PLAYER, DGROUP)))
            w.append((None, at + 0x3A, bytes([byte() % 10])))
    w.append((None, 0x7DB8, bytes(byte() for _ in range(4 + 12 * 0x1C))))
    w.append((None, 0x7F86, bytes(byte() for _ in range(8 * 0x16))))
    w.append((None, 0x0D5A, bytes(byte() for _ in range(2))))
    for o, n in [(0x05BE, 2), (0x05C6, 2), (0x05CA, 2), (0x0D49, 1), (0x0D39, 1), (0x1949, 2), (0x91F6, 4)]:
        v = bytes(byte() for _ in range(n))
        if o == 0x05BE:
            v = bytes([v[0] & 0x60, 0])
        elif o == 0x05CA:
            v = bytes([v[0] & 0x60, v[1] & 0x10])
        elif o == 0x05C6:
            v = bytes([v[0] % 40, 0])
        elif o == 0x0D49:
            v = bytes([v[0] % 8])
        elif o == 0x1949:
            v = bytes([0, 0]) if v[0] < 0xC0 else v
        w.append((None, o, v))
    return w


a = Arena(EXE)
for seg, off in [(0x23BA, 0x002A), (0x1194, 0xA36C)]:
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
a.w16(0x1042, CSEG)
a.w16(0x91F0, 0)
CLASSES = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
BASE_DS = a.seg_bytes(a.ds)
BASE_C = bytes(0x10000)


RELOC = [0x061C] + [0x0660 + i * 0x3C + 0x2A for i in range(16)]


def relocate():
    """Effect slots aimed at the player hold DGROUP's segment."""
    for o in RELOC:
        if a.r16(o) == DGROUP:
            a.w16(o, a.ds)


def unrelocated(mem):
    """DGROUP bytes with its segment image-relative where it is stored."""
    mem = bytearray(mem)
    for o in RELOC:
        if struct.unpack_from('<H', mem, o)[0] == a.ds:
            struct.pack_into('<H', mem, o, DGROUP)
    return bytes(mem)


SPECIAL = [3, 9] + list(a.mu.mem_read(a.lin(a.ds, 0x82BB), 4))


def special(k, ty):
    """A special case: the attacker is a creature of type `ty` (one that
    paralyses or spreads disease), the player has TIER_B 255 and no
    immunity flag."""
    return [(CSEG, k * RSIZE + 4, bytes([ty])), (CSEG, k * RSIZE + 0x40D, struct.pack('<H', 0x1000)),
            (None, PLAYER + 0x62, b'\xff'), (None, 0x05BE, b'\0\0')]


seed_rng = __import__('random').Random(0x1994_00D8)
for n in range(4000):
    seed = seed_rng.getrandbits(32)
    k = seed_rng.randrange(8)
    ty = SPECIAL[seed_rng.randrange(len(SPECIAL))] if n >= 3000 else None
    a.mu.mem_write(a.lin(a.ds, 0), BASE_DS)
    a.mu.mem_write(a.lin(CSEG, 0), BASE_C)
    for seg, off, data in setup(seed, k, CLASSES) + (special(k, ty) if ty is not None else []):
        a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
    a.w16(0x1400, k * RSIZE)
    a.w16(0x1402, PLAYER)
    relocate()
    before = unrelocated(a.seg_bytes(a.ds))
    try:
        a._setup([0, STUB_SEG], {})
        a._run(0x2AA1, 0x00D8, STUB_SEG, 0)
        after = unrelocated(a.seg_bytes(a.ds))
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x}'
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    if ty is None:
        out.write(f'creature | {seed:x} {k} | {res}\n')
    else:
        out.write(f'special | {seed:x} {k} {ty:x} | {res}\n')
out.close()
