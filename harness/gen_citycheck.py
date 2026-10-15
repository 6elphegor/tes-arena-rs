"""Emit test vectors for the CITYDATA check (src/city.rs
`citydata_check`, 242A:000E) from the original code.

Usage: gen_citycheck.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/citycheck.rs) the
save slot (1A6C), the province here (A46) and a CITYDATA.nn of nine
province blocks (4CCh bytes) whose names and locations are valid, then
now and then spoils one: a location's x at 140h/141h or y at C8h/C9h, a
location name's first byte one of 40h, 7Bh, 1Fh, 5Bh, 7Ah, 41h, a name of 13h or
14h letters with x 0 after it, or the province's own name. The message
box (1194:59C8) counts its calls at FFE0 and keeps SI at FFE2.

  citycheck | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes.
"""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault
from emu import changes, fnv, mine
from gen_attack import stream
from gen_automap import MemDos, digits
from gen_transit import counter

PROVINCE = 0x4CC


def name(byte, n):
    out = bytearray()
    for _ in range(n):
        c = byte()
        out.append(0x20 if c < 0x20 else 0x41 + c % 26)
    return out


def setup(seed):
    """(slot, here, files). Mirrored in tests/citycheck.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    slot = byte() % 10
    here = byte() % 9
    blocks = []
    for _ in range(9):
        b = bytearray(PROVINCE)
        n = name(byte, byte() % 16)
        b[0:len(n)] = n
        for loc in range(0x20):
            at = 0x1C + loc * 0x19
            n = name(byte, byte() % 16)
            b[at:at + len(n)] = n
            x, y = word() % 0x141, word() % 0xC9
            b[at + 0x14:at + 0x16] = x.to_bytes(2, 'little')
            b[at + 0x16:at + 0x18] = y.to_bytes(2, 'little')
        blocks.append(b)
    if byte() >= 0x80:
        p, kind, loc = byte() % 8, byte() % 5, byte() % 0x20
        b = blocks[p]
        at = 0x1C + loc * 0x19
        if kind == 0:
            b[at + 0x14:at + 0x16] = (0x140 + byte() % 2).to_bytes(2, 'little')
        elif kind == 1:
            b[at + 0x16:at + 0x18] = (0xC8 + byte() % 2).to_bytes(2, 'little')
        elif kind == 2:
            b[at] = (0x40, 0x7B, 0x1F, 0x5B, 0x7A, 0x41)[byte() % 6]
        elif kind == 3:
            n = 0x13 + byte() % 2
            for i in range(0x14):
                b[at + i] = 0x41 + i if i < n else 0
            # x after it 0: a 15h-th letter read would end the name there.
            b[at + 0x14:at + 0x16] = bytes(2)
        else:
            b[0] = 0x40
    files = {'CITYDATA.' + digits(slot).decode(): b''.join(bytes(b) for b in blocks)}
    return slot, here, files


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MemDos(EXE, os.path.dirname(EXE))
    a.mu.mem_write(a.lin(0x1000 + 0x1194, 0x59C8), counter(0xFFE0, True, keep=[(0xFFE2, 'si')]))
    base = a.snapshot()
    rng = random.Random(0x1994_242A)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mh = {}
        slot, here, files = setup(seed)
        a.mu.mem_write(a.lin(a.ds, 0x1A6C), bytes([slot]))
        a.mu.mem_write(a.lin(a.ds, 0x0A46), bytes([here]))
        a.files = {k: bytearray(d) for k, d in files.items()}
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x242A, 0x000E)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - 0x1000)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(changes(before, after)):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'citycheck | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
