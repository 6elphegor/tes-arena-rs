"""Emit test vectors for the travel map (src/travelmap.rs): the location
nearest the cursor (26F7:0B7A), a destination picked (26F7:03C4) and the
journey (26F7:07CD).

Usage: gen_travelmap.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/travelmap.rs) the
province's location lists (1445: 32 places, 1765: 16 dungeons, 19h
bytes each: a name, x, y at +14h, +16h, flags at +18h), the province
shown (47E1), the cursor (8F92, 8F94), the pick (A8AE) and the nearest
record (A8BA), the current location (0A46, 0A47, 0A4C, the record's +24h,
+26h), the wilderness bit (C39A 200h), the province bounds (143D..1443),
the destination (1929.., 12A8), the flags (6C28, 6C41, 5B1, 5C8), the
class and level (1A9, 1AA), the banks (929E, 1032) and the RNG's seed;
and the answers: the question's AX and the journey's days (returned in
AX, and as stored at 1927), with cases aimed at the edges.

The routines the travel map calls for drawing, files and the clock are
stubbed, each logging its call (an id and the registers that matter):
1 the message box (1194:59C8: SI, the segment: 0 DGROUP, 1 the data
segment), 2 the question (48DB: SI), 3 the fade (0000:5E3E), 4 a bank
mapped (5C86: BX), 5 a resource (56C8: SI, BX), 6 the days (1194:2E69,
which leaves them at 1927 too), 7 the shown bank (4D74: BX), 8 the
animation's load (0000:7428), 9 a frame (5526: CX), 10 minutes passing
(142D: EAX), 11 a province loaded (1194:2E14/2E18: AL), 12 a location
built (3AE8: AX), 13 a dungeon's (3B0C: AX, BX), 14 the arrival's log
(26F7:08F7), 15 the window closed (3159:0006). 1194:5E1D and 0000:9426
log nothing.

  travelmap | seed | op | result

Op: 0 nearest (result: BX, then the digests), 1 pick (AX), 2 go.
Results: "stuck", or AX/BX and the FNV-1a digests of the changed DGROUP
bytes and of the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EFLAGS
import unicorn.x86_const as X

DGROUP = 0x3BA0
DATA = 0x3276


def setup(seed):
    """(writes, op, answers). Mirrored in tests/travelmap.rs, draw for
    draw."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    op = byte() % 3
    # The locations: 48 records of 19h bytes from 1445 (to 1915).
    xy = []
    for i in range(48):
        rec = 0x1445 + 0x19 * i
        name = bytes(0x41 + byte() % 26 for _ in range(1 + byte() % 12)) + b'\0'
        w.append((rec, name))
        x, y = word() % 0x140, word() % 0xC8
        xy.append((x, y))
        w.append((rec + 0x14, pk(x)))
        w.append((rec + 0x16, pk(y)))
        w.append((rec + 0x18, bytes([byte()])))
    shown = byte() % 9 if byte() < 0xE0 else byte()
    w.append((0x47E1, bytes([shown])))
    w.append((0x8F92, pk(word() % 0x140)))
    w.append((0x8F94, pk(word() % 0xC8)))
    k = byte()
    pick = byte() % 0x20 if k < 0x80 else 0x20 + byte() % 0x10 if k < 0xF0 else word()
    w.append((0xA8AE, pk(pick)))
    near = byte() % 48
    w.append((0xA8BA, pk(0x1445 + 0x19 * near)))
    w.append((0x0A46, bytes([byte() % 9])))
    w.append((0x0A47, bytes([byte() % 4])))
    w.append((0x0A4C, bytes([byte()])))
    w.append((0x0A22 + 0x24, bytes([byte() % 9])))
    w.append((0x0A22 + 0x26, bytes(byte() for _ in range(4))))
    w.append((0xC39A, pk(word())))
    # The province's bounds: x, y, width, height.
    w.append((0x143D, pk(word() % 0x140)))
    w.append((0x143F, pk(word() % 0xC8)))
    w.append((0x1441, pk(word() % 0x140)))
    w.append((0x1443, pk(word() % 0xC8)))
    w.append((0x1929, bytes(byte() for _ in range(0x14))))
    for at in (0x193D, 0x1942, 0x1943, 0x1944):
        w.append((at, bytes([byte()])))
    w.append((0x1943, bytes([byte() % 4])))
    for at in (0x193E, 0x1940, 0x1945):
        w.append((at, pk(word())))
    w.append((0x12A8, pk(byte() % 9)))
    w.append((0x6C28, bytes([0 if byte() < 0x20 else 1])))
    w.append((0x6C41, bytes([0 if byte() < 0xE0 else 1])))
    w.append((0x05B1, pk(word() if byte() < 0x40 else 0)))
    w.append((0x05C8, pk(word())))
    w.append((0x0FC8, pk(word())))
    w.append((0x12A2, pk(word())))
    w.append((0x01A9, bytes([0x0E if byte() < 0x80 else byte()])))
    w.append((0x01AA, bytes([byte()])))
    w.append((0x929E, pk(word())))
    w.append((0x1032, pk(word())))
    w.append((0x40A0, bytes([byte()])))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    w.append((0x91F0, pk(0)))
    question = 0 if byte() < 0xC0 else word()
    k = byte()
    days = (0, 1, 2, 20, 21, 0x7CF, 0x7D0, 0x7D1, 0xFFFF)[byte() % 9] if k < 0x40 else word() % 0x40
    stored = days
    # Aimed at the edges random draws miss: the place clicked being the
    # player's own; the dungeon the player is in; a Spellsword's cut to
    # exactly nothing (43 days at level 42, the stored days at most 20 so
    # no jitter); the palette buffer's last byte set.
    k = byte()
    if k < 0x30:
        w.append((0x0A4C, bytes([((shown << 5) + pick) & 0xFF])))
    elif k < 0x60:
        w.append((0x0A47, bytes([3])))
        w.append((0x0A22 + 0x24, bytes([shown])))
        x, y = xy[near]
        w.append((0x0A22 + 0x26, pk(x) + pk(y)))
    elif k < 0x80:
        w.append((0x01A9, bytes([0x0E])))
        w.append((0x01AA, bytes([42])))
        days, stored = 43, byte() % 21
    w.append((0x929E + 0x2FF, bytes([1 + byte() % 0xFF])))
    return w, op, (question, days, stored)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    # DGROUP's relocated words put back image-relative, as the port has
    # them.
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    state = {}
    reg = lambda r: mu.reg_read(getattr(X, 'UC_X86_REG_' + r.upper()))
    wreg = lambda r, v: mu.reg_write(getattr(X, 'UC_X86_REG_' + r.upper()), v)

    def stub(seg, off, far, fn):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb' if far else b'\xc3')
        lin = a.lin(LOAD_SEG + seg, off)

        def run(mu, addr, size, _):
            wreg('eflags', reg('eflags') & ~1)
            fn()
        mu.hook_add(UC_HOOK_CODE, run, None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def segof():
        return 0 if reg('ds') == a.ds else 1

    def days():
        a.w16(0x1927, state['answers'][2])
        wreg('ax', state['answers'][1])
        log(6)

    stub(0x1194, 0x5E1D, True, lambda: None)
    stub(0x1194, 0x59C8, True, lambda: log(1, reg('si'), segof()))
    stub(0x1194, 0x48DB, True, lambda: (log(2, reg('si')), wreg('ax', state['answers'][0])))
    stub(0, 0x5E3E, True, lambda: log(3))
    stub(0, 0x9426, True, lambda: None)
    stub(0, 0x5C86, True, lambda: log(4, reg('bx')))
    stub(0, 0x56C8, True, lambda: log(5, reg('si'), reg('bx')))
    stub(0x1194, 0x2E69, True, days)
    stub(0x1194, 0x4D74, True, lambda: log(7, reg('bx')))
    stub(0, 0x7428, True, lambda: log(8))
    stub(0, 0x5526, True, lambda: log(9, reg('cx')))
    stub(0, 0x9466, True, lambda: None)
    stub(0, 0x142D, True, lambda: log(10, reg('eax')))
    stub(0x1194, 0x2E14, True, lambda: log(11, reg('al')))
    stub(0x1194, 0x2E18, False, lambda: log(11, reg('al')))
    stub(0x1194, 0x3AE8, True, lambda: log(12, reg('ax')))
    stub(0x1194, 0x3B0C, True, lambda: log(13, reg('ax'), reg('bx')))
    stub(0x26F7, 0x08F7, False, lambda: log(14))
    stub(0x3159, 0x0006, True, lambda: log(15))
    base = bytes(a.seg_bytes(a.ds))
    rng = random.Random(0x1994_26F7)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base)
        writes, op, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        state.update(log=[], answers=answers)
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                r = a.call_near(0x26F7, 0x0B7A, ax=a.r16(0x8F92), bx=a.r16(0x8F94))
                head = f"{r['bx']:x}"
            elif op == 1:
                r = a.call_near(0x26F7, 0x03C4, ax=a.r16(0xA8AE))
                head = f"{r['ax']:x}"
            else:
                a.call_far(0x26F7, 0x07CD)
                head = '-'
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log'])
            res = f'{head} {fnv(changes(before, after)):x} {fnv(lg):x}'
            if os.environ.get('DUMP') == f'{seed:x}':
                print(state['log'], file=sys.stderr)
                c = changes(before, after)
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'travelmap | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
