"""Emit test vectors for a city entered (src/cityinfo.rs `city_entered`,
0000:46B2..4766, the outdoor part of the hook CD42 after its light).

Usage: gen_entered.py ACD_unpacked.EXE OUT

Stubbed: 1194:3EBF (counts its calls at DGROUP FFE0 and adds AL to
FFE6) and 2294:04C0, the town's music (counts at FFE2). The place now is
the one last seen (CCA8 = 0FC8), so 1194:9358 runs and returns at once
(its making of quests has its own vectors).

A case draws (`setup`, mirrored in tests/entered.rs) the wilderness bit
(C39A 200h), 5CA bit 4000h, the place (A4C), the counts (1387, 138B,
1389, sometimes 0), the palace's square (138D), the lists' segment
(their entries, some made to match the quests), and the four quests:
their kind and sort (+4), place (+24h), pair (+16h) and goal (+14h).

  entered | seed | result

Results: "stuck", or the FNV-1a digests of the changed bytes of DGROUP
and of the lists' segment.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

LSEG = 0x6C00


def setup(seed):
    """(ds writes, lists' segment bytes). Mirrored in tests/entered.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0xC39A, pk(0x200 if byte() < 0x20 else 0)))
    w.append((0x05CA, pk(0x4000 if byte() < 0x40 else 0)))
    place = byte()
    w.append((0x0A4C, bytes([place])))
    seen = word()
    w.append((0xCCA8, pk(seen)))
    w.append((0x0FC8, pk(seen)))
    counts = []
    for at in (0x1387, 0x138B, 0x1389):
        b = byte()
        n = 0 if b < 0x10 else b % 7
        counts.append(n)
        w.append((at, pk(n)))
    w.append((0x138D, pk(word())))
    fill = word()
    lseg = bytearray((fill + 3 * i) & 0xFF for i in range(0x10000))
    pairs = {0: [], 1: [], 3: []}
    for sort, base in ((0, 0), (1, 0x100), (3, 0x180)):
        for k in range(8):
            sq, pair = word(), word()
            struct.pack_into('<HH', lseg, base + 4 * k, sq, pair)
            pairs[sort].append(pair)
    w.append((0x1AB5, pk(0) + pk(LSEG)))
    w.append((0x1008, pk(0x100) + pk(LSEG)))
    w.append((0x1004, pk(0x180) + pk(LSEG)))
    for q in range(4):
        si = 0x0E6E + q * 0x34
        kind = (1 if byte() < 0x90 else byte() % 4) << 6
        sort = (0, 1, 2, 3, 6, 4, 0, 1, 3)[byte() % 9]
        low = word() & 0x0F3F
        w.append((si + 4, pk(sort << 12 | low | kind)))
        w.append((si + 0x24, bytes([place if byte() < 0xC0 else byte()])))
        lst = pairs.get(sort, [])
        b = byte()
        pair = lst[b % 8] if lst and b < 0x90 else word()
        w.append((si + 0x16, pk(pair)))
        w.append((si + 0x14, pk(word())))
    return w, bytes(lseg)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    # 1194:3EBF: INC WORD [FFE0]; ADD [FFE6], AL; RETF.
    a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, 0x3EBF), b'\xff\x06\xe0\xff\x00\x06\xe6\xff\xcb')
    # 2294:04C0: INC WORD [FFE2]; RETF.
    a.mu.mem_write(a.lin(LOAD_SEG + 0x2294, 0x04C0), b'\xff\x06\xe2\xff\xcb')
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_46B2)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, lseg = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(LSEG, 0), lseg)
        before = [a.seg_bytes(a.ds), a.seg_bytes(LSEG)]
        try:
            a.call_far(0, 0x46B2, eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, eflags=2)
            if a.fault is not None:
                raise Fault(a.fault)
            after = [a.seg_bytes(a.ds), a.seg_bytes(LSEG)]
            res = ' '.join(f'{fnv(changes(x, y)):x}' for x, y in zip(before, after))
        except (Fault, AssertionError, UcError):
            res = 'stuck'
        out.write(f'entered | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
