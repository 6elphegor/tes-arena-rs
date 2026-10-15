"""Emit test vectors for CITYDATA's writes (src/city.rs
`city_save_province`, `city_save_entry`, and src/quest.rs
`quest_dungeon_entry` with its default hooks) from the original code.

Usage: gen_citydata.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/citydata.rs) the
save slot (1A6C), a CITYDATA.nn of nine random province blocks (4CCh
bytes each; the dungeon entries' flags +18h with bit 1 now and then, a
name or none), the province here (A46), the province's data at 1429, the
RNG's seed (91F6; table mode off, 91F0) and a name at 929E, then runs,
with the files in memory (gen_automap.py `MemDos`):

  op 0  26F7:0C0A with AX the province, BX the mode (0..5), CX an index,
        SI the name: the province loaded (1194:2E14), an entry picked,
        written back to CITYDATA.nn and the place's province loaded again
  op 1  26F7:0CF1 with AX the province: the province's block written

  citydata | seed | result

Results: "fault" / "hang", or the FNV-1a digests of AX (op 0; 0 for op
1), the changed DGROUP bytes and the slot's CITYDATA.nn.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_automap import MemDos, digits

PROVINCE = 0x4CC


def setup(seed):
    """(op, ax, bx, cx, slot, writes, files). Mirrored in
    tests/citydata.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() & 1
    slot = byte() % 10
    province = byte() % 9
    here = byte() % 9
    mode = byte() % 6
    index = byte() % 0x10
    data = bytearray()
    for _ in range(9):
        block = bytearray(byte() for _ in range(PROVINCE))
        # The dungeon entries from 1765 - 1429 = 33Ch, 19h bytes each:
        # flags +18h, a name at +0 (or none) ending in a NUL by +13h.
        for e in range(0x10):
            at = 0x33C + e * 0x19
            if at + 0x19 > PROVINCE:
                break
            block[at + 0x18] = byte() & 0xFD | (2 if byte() < 0x40 else 0)
            n = byte() % 12 if byte() < 0xC0 else 0
            for i in range(0x13):
                block[at + i] = (0x41 + byte() % 26) if i < n else 0
        data += block
    w = []
    w.append((0x1A6C, bytes([slot])))
    w.append((0x0A46, bytes([here])))
    w.append((0x1429, bytes(byte() for _ in range(PROVINCE))))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    w.append((0x91F0, pk(0)))
    n = 1 + byte() % 12
    w.append((0x929E, bytes(0x61 + byte() % 26 for _ in range(n)) + b'\0'))
    files = {'CITYDATA.' + digits(slot).decode(): bytes(data)}
    return op, province, mode, index, slot, w, files


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MemDos(EXE, os.path.dirname(EXE))
    base = a.snapshot()
    rng = random.Random(0x1994_0C0A)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mh = {}
        op, ax, bx, cx, slot, writes, files = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        a.files = {k: bytearray(d) for k, d in files.items()}
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                r = a.call_far(0x26F7, 0x0C0A, ax=ax, bx=bx, cx=cx, si=0x929E)
            else:
                a.call_far(0x26F7, 0x0CF1, ax=ax)
                r = 0
            # The file name's far pointer (the emulator relocates DGROUP).
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            if os.environ.get('DUMP') == f'{seed:x}':
                print(' '.join(f'{i:04x}:{before[i]:02x}>{after[i]:02x}' for i in range(0x10000) if before[i] != after[i]), file=sys.stderr)
            name = 'CITYDATA.' + digits(slot).decode()
            res = f'{fnv(struct.pack("<H", r & 0xFFFF)):x} {fnv(changes(before, after)):x} {fnv(bytes(a.files.get(name, b""))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'citydata | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
