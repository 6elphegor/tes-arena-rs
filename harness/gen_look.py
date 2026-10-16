"""Emit test vectors for looking at a square (src/look.rs `look`,
2826:0C15 with 0D12 and the names 1194:E539, E5BE, E626) from the
original code.

Usage: gen_look.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/look.rs) the wild's
flag (C39A bit 200h), the town's (C398 bit 4), the square looked at (AX:
its row in AH, its column in AL) and the one picked (CB92), the map's
word there (D806's plane at MSEG: none, a wall of a trigger's kind, a
random word, or a flat's), the 16 triggers (C917), the wild's kinds of
place (67FE), the three lists of places (1AB5, 1004, 1008 at PSEG, 1..5
entries, about half of them at the square), the province (A46) and the
place's region (A4C), and CLICKED (CB5E).

Stubs, each a code hook at its first instruction patched to RETF, log
their calls: the substitutions (2DA2:000A: 8, DI; DI moved to the
string's NUL, as the port's default does) and the note (23F1:00A3: 5,
SI, AX, BX).

  look | seed | result

Results: "stuck" (the original faults or runs past the cap), or the
FNV-1a digests of the changed DGROUP bytes (offset, value) and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_SI, UC_X86_REG_DI, UC_X86_REG_ES
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream

MSEG = 0x7000
PSEG = 0x8000


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes and the square
    looked at (AX). Mirrored in tests/look.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    W = lambda off, v: w.append((None, off, pk(v)))
    B = lambda off, v: w.append((None, off, bytes([v & 0xFF])))
    W(0xC39A, 0x200 if byte() < 0x60 else 0)
    W(0xC398, 4 if byte() < 0xD0 else 0)
    col, row = byte() & 0x7F, byte() & 0x7F
    if byte() < 0x10:
        col = byte()
    ax = row << 8 | col
    W(0xCB92, row << 8 | (col << 1) & 0xFF if byte() < 0xE0 else word())
    triggers = [byte() % 0x40 for _ in range(16)]
    for i, t in enumerate(triggers):
        B(0xC917 + i, t)
    for i in range(16):
        B(0x67FE + i, (0xFF, 2, 5, 0, 1, 3, 4)[byte() % 7])
    k = byte() % 4
    if k == 1:
        t = triggers[byte() % 16] if byte() < 0xE0 else byte() % 0x40
        v = t | (byte() & 0xC0)
        v = v << 8 | v
    elif k == 2:
        v = word()
    elif k == 3:
        v = 0x8000 | word()
    else:
        v = 0
    w.append((MSEG, row << 8 | (col << 1) & 0xFF, pk(v)))
    for i, (ptr, cnt) in enumerate(((0x1AB5, 0x1387), (0x1004, 0x1389), (0x1008, 0x138B))):
        n = 1 + byte() % 5
        W(cnt, n)
        W(ptr, 0x100 * i)
        W(ptr + 2, PSEG)
        for e in range(n):
            if byte() < 0x50:
                c, r = col, row
            else:
                c, r = byte() & 0x7F, byte() & 0x7F
            w.append((PSEG, 0x100 * i + 4 * e, bytes([c, r, byte() % 8, byte() % 6])))
    B(0x0A46, byte() % 9)
    B(0x0A4C, byte())
    W(0xCB5E, word())
    return w, ax


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    log = []

    def stub(seg, off, fn):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def expand():
        di = mu.reg_read(UC_X86_REG_DI)
        log.extend([8, di])
        es = mu.reg_read(UC_X86_REG_ES)
        while mu.mem_read(a.lin(es, di), 1)[0] != 0:
            di = (di + 1) & 0xFFFF
        mu.reg_write(UC_X86_REG_DI, di)

    def norm(b):
        """DGROUP's bytes, the message's segment (061C) DGROUP's."""
        b = bytearray(b)
        if struct.unpack_from('<H', b, 0x061C)[0] == a.ds:
            struct.pack_into('<H', b, 0x061C, DGROUP)
        return b

    stub(0x2DA2, 0x000A, expand)
    stub(0x23F1, 0x00A3, lambda: log.extend([5, mu.reg_read(UC_X86_REG_SI), mu.reg_read(UC_X86_REG_AX), mu.reg_read(UC_X86_REG_BX)]))
    a.w16(0xD806, MSEG)
    base_ds = a.seg_bytes(a.ds)
    zeros = bytes(0x10000)
    rng = random.Random(0x1994_0C15)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        mu.mem_write(a.lin(MSEG, 0), zeros)
        mu.mem_write(a.lin(PSEG, 0), zeros)
        writes, ax = setup(seed)
        for seg, off, data in writes:
            mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        log.clear()
        before = norm(a.seg_bytes(a.ds))
        try:
            a.call_near(0x2826, 0x0C15, eax=ax)
            after = norm(a.seg_bytes(a.ds))
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{fnv(changes(before, after)):x} {fnv(lg):x}'
            if os.environ.get('DUMP') == f'{seed:x}':
                for i in range(0x10000):
                    if before[i] != after[i]:
                        print(f'{i:04x} {before[i]:02x} {after[i]:02x}', file=sys.stderr)
                print('log', [hex(v) for v in log], file=sys.stderr)
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'look | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
