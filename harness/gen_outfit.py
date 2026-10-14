"""Emit test vectors for a person's graphics list (src/outfit.rs
`person_outfit`, 2686:0423).

Usage: gen_outfit.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/outfit.rs) a
record in the creature segment (one of the eight): +9 (a person, mostly
set), the class (+5: mages, the monk's 4Ch, the barbarian's 0Fh, or
any), +7 (a woman), the flags at +40Dh, and its 40 item slots from +6Eh:
some occupied, of armour (cuirasses, shields and the rest, by kind) and
weapons (some broken, FFFFh), equipped or not; the weapon class at
0D49, the armour counters (12A8..12AC), and for a bare hand's gauntlet
search (in the creature segment) the count at 129D and gauntlets in the
slots from FFFFh.

  outfit | seed | list | result

List: the names built at 1044, joined by commas. Results: "fault" /
"hang", or the FNV-1a digest of the changed DGROUP bytes then the
changed creature segment bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import changes
from emu import mine
from gen_attack import fnv, stream

CSEG = 0x6000


def setup(seed):
    """(writes, record). Mirrored in tests/outfit.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    rec = (byte() % 8) * 0x41E
    w.append((CSEG, rec + 9, pb(0 if byte() < 0x20 else 1 + byte() % 0x40)))
    k = byte() % 5
    cls = (0x20 | byte() & 0x1F, 0x4C, 0x0F, byte(), byte() & 0x1F)[k]
    w.append((CSEG, rec + 5, pb(cls)))
    w.append((CSEG, rec + 7, pb(byte() & 1)))
    w.append((CSEG, rec + 0x40D, pk(word())))
    w.append((None, 0x0D49, pb(byte())))
    n = byte() % 12
    for i in range(40):
        at = rec + 0x6E + 0x13 * i
        if i >= n or byte() < 0x20:
            w.append((CSEG, at + 0xE, pb(byte() & 0x3F & ~0x40 if byte() < 0x80 else 0)))
            continue
        kind = byte() % 3
        if byte() < 0x90:
            # Armour: cuirass (0), shield (7..10), or another piece.
            t = (0, 7 + byte() % 4, 1 + byte() % 6, byte())[byte() % 4]
            flags = 0x40 | kind << 4
        else:
            t = byte()
            flags = 0x40 | 1 << 2 | kind << 4
        if byte() < 0xA0:
            flags |= 0x80
        w.append((CSEG, at, pb(t)))
        w.append((CSEG, at + 6, pk(0xFFFF if byte() < 0x10 else word())))
        w.append((CSEG, at + 0xE, pb(flags)))
    # The counters as left before, and what a bare hand's gauntlet search
    # meets: the count it reads in the creature segment and slots from
    # FFFFh there.
    w.append((None, 0x12A8, bytes(byte() for _ in range(5))))
    w.append((CSEG, 0x129D, pb(byte() % 8)))
    for k in range(4):
        at = (0xFFFF + 0x13 * k) & 0xFFFF
        w.append((CSEG, at, pb(3 if byte() < 0x80 else byte())))
        w.append((CSEG, (at + 0xE) & 0xFFFF, pb(0xC4 | (byte() % 3) << 4)))
    return w, rec


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0x1042, CSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_0423)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(CSEG, 0), zero)
        writes, rec = setup(seed)
        for seg, off, data in writes:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before, cbefore = a.seg_bytes(a.ds), a.seg_bytes(CSEG)
        names = '-'
        try:
            a.call_far(0x2686, 0x0423, ds=CSEG, si=rec)
            after, cafter = a.seg_bytes(a.ds), a.seg_bytes(CSEG)
            ch = lambda x, y: changes(x, y)
            res = f'{fnv(ch(before, after) + ch(cbefore, cafter)):x}'
            lst, at = [], 0x1044
            while after[at]:
                e = after.index(b'\0', at)
                lst.append(after[at:e].decode('latin-1'))
                at = e + 1
            names = ','.join(lst) or '-'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'outfit | {seed:x} | {names} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
