"""Emit test vectors for the main quest's next step offered (src/quest.rs
`quest_offer`, 1194:CD9A, with the step's roll CE7F and its piece's check
1194:7C83).

Usage: gen_questoffer.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/questoffer.rs) the
player's items (0212: each slot's type, a piece with bit 80h at times,
and its flags with the occupied bit), the class (1A9), the item buffer
(13DA), the main quest's block (0FDD..0FEF: flags 0FE6, the steps done
0FE7, sometimes all or all but one), the province (A46), the time (5CC),
the table of words the step's dword comes from (its far pointer at 1AB5
set to 7000:0000, its count at 1387, 0 at times) and the RNG's seed
(91F6). Nothing is stubbed.

  questoffer | seed | result

Results: "fault" / "hang", or the carry and the FNV-1a digest of the
changed DGROUP bytes (offset, value).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn.x86_const import UC_X86_REG_EFLAGS
from emu import Arena, Fault
from emu import changes, fnv, mine
from gen_attack import stream

TSEG = 0x7000


def setup(seed):
    """(DGROUP writes, the table's bytes). Mirrored in tests/questoffer.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    pieces = byte() < 0x30
    # No slot occupied at times: 1194:CDEA's `loop` then runs 65536 times.
    empty = byte() < 0x20
    for i in range(40):
        at = 0x0212 + i * 0x13
        t = byte()
        t = t | 0x80 if pieces and byte() < 0x20 else t & 0x7F
        w.append((at, bytes([t])))
        f = byte()
        w.append((at + 0x0E, bytes([f & ~0x40 if empty else f])))
    w.append((0x01A9, bytes([byte() % 18 | byte() & 0xE0])))
    w.append((0x13DA, bytes(byte() for _ in range(0x13))))
    block = bytearray(byte() for _ in range(0x13))
    block[9] = [0x00, 0x04, 0x0C][byte() % 3] if byte() < 0x80 else byte()
    k = byte() % 4
    done = [0xFFFF, 0xFFFF ^ 1 << byte() % 16, word(), 0][k]
    block[10:12] = pk(done)
    w.append((0x0FDD, bytes(block)))
    w.append((0x0A46, bytes([byte() % 8])))
    w.append((0x05CC, struct.pack('<I', byte() << 16 | word())))
    w.append((0x1387, pk(0 if byte() < 0x08 else 1 + byte() % 32)))
    w.append((0x1AB5, struct.pack('<HH', 0, TSEG)))
    w.append((0x91F6, struct.pack('<I', word() | word() << 16)))
    table = bytes(byte() for _ in range(0x80))
    return w, table


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_CD9A)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, table = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(TSEG, 0), table)
        before = a.seg_bytes(a.ds)
        try:
            # CD9A returns with a near RET: run from CD9A to a RET's
            # landing at 1194:FFF0 (`call_near`).
            a.call_near(0x1194, 0xCD9A)
            cf = mu.reg_read(UC_X86_REG_EFLAGS) & 1
            after = a.seg_bytes(a.ds)
            res = f'{cf} {fnv(changes(before, after)):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'questoffer | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
