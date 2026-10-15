"""Emit test vectors for the side quests made on entering a place
(src/sidequest.rs `quests_place`, 1194:9358 with 9462, 9D64 and their
makers).

Usage: gen_sidequest.py ACD_unpacked.EXE OUT

Stubbed: 1194:3EC3, the place's buildings listed (file I/O; it counts
its calls at DGROUP FFE0 and adds AL to the byte at FFE6), 1194:2E18,
the province loaded (FFE2, AL to FFE7) and 1194:2E69, the journey's days
(far; FFE4; the days are what the case put at 1927). The PC's timer
(0000:9D60) reads the case's dword at 0:046C. The heap runs for real,
set up afresh each case at A000..F000.

A case fills from a seed (`setup`, mirrored in tests/sidequest.rs) the
place now and the one last seen (equal at times), the location and
province (8 at times), the level, A47, A52, the time (past FFFFh days at
times), the RNG's seed, the eight records (random bytes, their kinds
drawn), the failed quests' table (times around now), the places' lists
(far pointers into 7000:0000, their counts, 0 at times; entries' kinds
sometimes from the excluded ones), 138D/138F, the province's places at
1445, the journey's days at 1927, the timer, and the bytes the heap's
block will hold (the other place's lists, as the stub leaves them).

  sidequest | seed | result

Results: "stuck" (the original faults or runs past the cap), or the
FNV-1a digests of the changed DGROUP bytes (offset, value; but for the
SP 9462 and 9D64 store at 6CC2 and 6DCA, never read) and of the heap's
changed bytes from its zeros (linear address dword, value).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

TSEG = 0x7000
HEAP_START, HEAP_TOP = 0xA000, 0xF000
# 3EC3 / 2E18 (near): count, add AL; ret. 2E69 (far): count; retf.
STUB_INFO = bytes([0xFF, 0x06, 0xE0, 0xFF, 0x00, 0x06, 0xE6, 0xFF, 0xC3])
STUB_PROV = bytes([0xFF, 0x06, 0xE2, 0xFF, 0x00, 0x06, 0xE7, 0xFF, 0xC3])
STUB_DAYS = bytes([0xFF, 0x06, 0xE4, 0xFF, 0xCB])
EXCLUDED = [0x00, 0x01, 0x02, 0x08, 0x0F, 0x12, 0x13]


def heap_changes(old, new):
    """The heap's changed bytes: (linear address dword, new byte)."""
    out = []
    for c in range(0, len(new), 256):
        if old[c:c + 256] != new[c:c + 256]:
            for i in range(c, c + 256):
                if old[i] != new[i]:
                    out.append(struct.pack('<IB', HEAP_START * 16 + i, new[i]))
    return b''.join(out)


def setup(seed):
    """(DGROUP writes, the lists' bytes, the timer). Mirrored in
    tests/sidequest.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    place = word()
    w.append((0xCCA8, pk(place)))
    w.append((0x0FC8, pk(place if byte() < 0x20 else word())))
    w.append((0x0A4C, bytes([byte()])))
    w.append((0x0A46, bytes([8 if byte() < 0x20 else byte() % 8])))
    w.append((0x01AA, bytes([byte() if byte() < 0x10 else byte() % 40])))
    w.append((0x0A47, bytes([byte() % 4])))
    w.append((0x0A52, pd(dword())))
    t = word() << 16 | word() if byte() < 0x10 else byte() << 16 | word()
    w.append((0x05CC, pd(t)))
    w.append((0x91F6, pd(dword())))
    quests = []
    for base, size in ((0x0DCA, 0x29), (0x0E6E, 0x34)):
        for i in range(4):
            rec = bytearray(byte() for _ in range(size))
            kind = [0, 0, 1, 2, 3][byte() % 5]
            k = word() & ~0xC0 | kind << 6
            rec[4:6] = pk(k)
            w.append((base + i * size, bytes(rec)))
            if size == 0x34:
                quests.append((k >> 9 & 7, rec[0x25]))
    for i in range(8):
        # Half the entries name one of the quests (its stage and place),
        # a quarter forgotten exactly now.
        stage, place = byte() % 8, byte()
        if byte() < 0x80:
            stage, place = quests[byte() % 4]
        k = byte()
        when = t if k < 0x40 else (t + (word() - 0x8000)) if k < 0xC0 else dword()
        w.append((0x0A7E + i * 6, bytes([stage, place]) + pd(when)))
    for at in (0x1387, 0x1389, 0x138B):
        w.append((at, pk(0 if byte() < 0x10 else 1 + byte() % 12)))
    w.append((0x1AB5, struct.pack('<HH', 0x000, TSEG)))
    w.append((0x1004, struct.pack('<HH', 0x100, TSEG)))
    w.append((0x1008, struct.pack('<HH', 0x180, TSEG)))
    w.append((0x138D, pk(word())))
    w.append((0x138F, pk(word())))
    w.append((0x1445, bytes(byte() for _ in range(16 * 0x19))))
    w.append((0x1927, pk(byte() % 40)))
    lists = bytearray(byte() for _ in range(0x200))
    for i in range(0x80):
        if byte() < 0x60:
            lists[i * 4 + 2] = EXCLUDED[byte() % 7]
    ticks = dword() if byte() < 0xF0 else 0
    # What the heap's block (taken from the free block's start, A001:0)
    # holds before the stubbed 3EC3 leaves it: the other place's lists.
    block = bytearray(byte() for _ in range(0x300))
    for i in range(0xC0):
        if byte() < 0x60:
            block[i * 4 + 2] = EXCLUDED[byte() % 7]
    return w, bytes(lists), ticks, bytes(block)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    for off, code in ((0x3EC3, STUB_INFO), (0x2E18, STUB_PROV), (0x2E69, STUB_DAYS)):
        mu.mem_write(a.lin(LOAD_SEG + 0x1194, off), code)
    base_ds = a.seg_bytes(a.ds)
    heap_zero = bytes((HEAP_TOP - HEAP_START) * 16)
    rng = random.Random(0x1994_9358)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        mu.mem_write(HEAP_START * 16, heap_zero)
        a.w16(0x159, HEAP_START)
        a.w16(0x15D, HEAP_TOP)
        a.call_near(0, 0x00CC)
        writes, lists, ticks, block = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(TSEG, 0), lists)
        mu.mem_write(0x46C, struct.pack('<I', ticks))
        mu.mem_write((HEAP_START + 1) * 16, block)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x9358)
            # 9462 and 9D64 store SP at 6CC2 and 6DCA, which nothing
            # reads; the port has no SP: left out.
            after = bytearray(a.seg_bytes(a.ds))
            for at in (0x6CC2, 0x6CC3, 0x6DCA, 0x6DCB):
                after[at] = before[at]
            heap = bytes(mu.mem_read(HEAP_START * 16, len(heap_zero)))
            res = f'{fnv(changes(before, after)):x} {fnv(heap_changes(heap_zero, heap)):x}'
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'sidequest | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
