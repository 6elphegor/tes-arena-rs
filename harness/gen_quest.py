"""Emit test vectors for the quests' clock (src/quest.rs `quests_hourly`,
1194:98E8, and `quests_daily`, 1194:99F8, with the failure 1194:9A65,
the quest item freed 9C74 and the message 9C96/9C9B).

Usage: gen_quest.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/quest.rs) the time
(5CC), the item list (0FD4, 9 bytes, and 0FDD past it), the four quests
(0E6E, 34h bytes each) and the four records 99F8 walks (0DCA, 29h each):
their bytes at random, then the kind (bits 6-7 of +4), the stage (bits
9-11) and the deadline (+0Ch) aimed at the day's boundaries around the
time, the item (+1Ch) often one in the list; the failures' table (0A7E,
8 of 6 bytes, some free), 1013, 8A56, 0D39, 5C8 and the province's data
(1429, 4CCh bytes).

Stubs, each a code hook at its first instruction patched to RET or RETF,
log their calls: the text (1194:A374: 1, [137A], [137E]), the province's
load (1194:2E18: 2, AL) and its write back (26F7:0CF1: 3, AX). The fonts
(0000:9AE6, 9AF8) and the message box (1194:59CC) return at once.

  quest | seed | op | result

Op: 0 hourly (98E8), 1 daily (99F8). Results: "fault" / "hang", or the
FNV-1a digests of the changed DGROUP bytes (offset, value) and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

QUESTS = 0x0E6E
OTHERS = 0x0DCA
DAY = 0x5A0
# The deadline's offset from the time, the last one any word * 16.
OFFSETS = [-DAY - 1, -DAY, -DAY + 1, -1, 0, 1, DAY - 1, DAY, DAY + 1]


def setup(seed):
    """(op, writes). Mirrored in tests/quest.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    op = byte() & 1
    t = byte() << 16 | word()
    w.append((0x05CC, struct.pack('<I', t)))
    items = [0xFF if byte() < 0x60 else byte() % 0x50 for _ in range(9)]
    w.append((0x0FD4, bytes(items)))
    w.append((0x0FDD, bytes([byte()])))

    def record(at, size):
        w.append((at, bytes(byte() for _ in range(size))))
        flags = word()
        kind = [1, 1, 1, 0, 2, 3][byte() % 6]
        stage = byte() % 8
        flags = flags & 0xF13F | kind << 6 | stage << 9
        w.append((at + 4, struct.pack('<H', flags)))
        k = byte() % 11
        if k < 9:
            d = t + OFFSETS[k]
        elif k == 9:
            d = t + (word() - 0x8000) * 16
        else:
            d = byte()
        w.append((at + 0xC, struct.pack('<I', d & 0xFFFFFFFF)))
        item = items[byte() % 9] if byte() < 0x80 else byte()
        w.append((at + 0x1C, bytes([item])))

    for i in range(4):
        record(QUESTS + i * 0x34, 0x34)
    for i in range(4):
        record(OTHERS + i * 0x29, 0x29)
    for i in range(8):
        first = 0xFF if byte() < 0x40 else byte()
        w.append((0x0A7E + i * 6, bytes([first] + [byte() for _ in range(5)])))
    w.append((0x1013, bytes([0 if byte() < 0x80 else byte()])))
    w.append((0x8A56, bytes([byte()])))
    w.append((0x0D39, bytes([byte()])))
    w.append((0x05C8, struct.pack('<H', word())))
    w.append((0x1429, bytes(byte() for _ in range(0x4CC))))
    return op, w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    RETF, RET = b'\xcb', b'\xc3'
    log = []

    def patch(seg, off, code):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), code)

    def stub(seg, off, far, fn):
        patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    for seg, off, code in [(0, 0x9AE6, RETF), (0, 0x9AF8, RETF), (0x1194, 0x59CC, RET)]:
        patch(seg, off, code)
    ax = lambda: mu.reg_read(UC_X86_REG_AX)
    stub(0x1194, 0xA374, False, lambda: log.extend([1, a.r16(0x137A), mu.mem_read(a.lin(a.ds, 0x137E), 1)[0]]))
    stub(0x1194, 0x2E18, False, lambda: log.extend([2, ax() & 0xFF]))
    stub(0x26F7, 0x0CF1, True, lambda: log.extend([3, ax()]))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_98E8)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        op, writes = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, (0x98E8, 0x99F8)[op])
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{fnv(changes(before, after)):x} {fnv(lg):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'quest | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
