"""Emit test vectors for a place's buildings listed (src/cityinfo.rs
`city_info`, 1194:3EBF / 3EC3, with 3B84, 3D91, 3DF2, 4189 and 40CF).

Usage: gen_cityinfo.py ACD_unpacked.EXE GAME_DIR OUT

The original runs on the game's own files: GLOBAL.BSA opened as the game
opens it (0000:6F3A), CITYDATA.65 and the blocks' .MIF files read through
DOS, TMP2.$$$ (3DF2's copy of the screen) created in memory (gen_store's
StoreDos). EMS is off; the heap runs for real.

A case draws (`setup`, mirrored in tests/cityinfo.rs, draw for draw) the
current place's record (its province, id, plan size, X and Y), the seed,
the path buffers' toggle (1AEA), the plan's cells, AX (the current place,
another of the province, or the Imperial City or another province's),
the five flat kinds, the map (D806) as a pattern with squares of each
kind planted (a quarter of the cases with 24 to 29 temples, about all
the temples' pairs) and the screen (9030) as a pattern.

  cityinfo | seed | result

Results: "stuck" (a fault, a hang, an invalid instruction), or the FNV-1a
digests of the changed bytes of DGROUP, the lists' segment, the screen,
the heap's first 64 KB, and of the files written (names and contents).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from unicorn.x86_const import *
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream
from gen_store import StoreDos

WSEG = 0x7C00
FSEG = 0x5C00
LSEG = 0x6C00
HEAP = 0xA000


def setup(seed):
    """(ds writes, ax, map words, frame bytes). Mirrored in
    tests/cityinfo.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    k = byte() % 8
    p0 = byte() % 8
    p = 8 if byte() < 0x10 else p0
    cur = (p << 5 | byte() % 32) & 0xFF
    d = byte()
    dim = d % 6 + 1 if d < 0xF0 else d % 10
    x, y = word(), word()
    seed_lo, seed_hi = word(), word()
    toggle = byte() & 1
    w.append((0x0A46, bytes([p])))
    w.append((0x0A4C, bytes([cur])))
    w.append((0x0A43, bytes([dim])))
    w.append((0x0A48, pk(x)))
    w.append((0x0A4A, pk(y)))
    w.append((0x91F6, pk(seed_lo) + pk(seed_hi)))
    w.append((0x91F0, pk(0)))
    w.append((0x1AEA, bytes([toggle])))
    w.append((0x1A6C, bytes([65])))
    plan = []
    for _ in range(81):
        b = byte()
        plan.append(1 if b < 0x20 else b)
    w.append((0x486B, bytes(plan)))
    if k < 4:
        ax = byte() << 8 | cur
    elif k < 7:
        ax = 0x100 | ((p << 5 | byte() % 32) & 0xFF)
    else:
        b = byte()
        ax = 0x100 if b < 0x40 else 0x100 | b
    kinds = [0x40 + byte() % 12 for _ in range(5)]
    for at, v in zip((0xC918, 0xC917, 0xC91A, 0xC919, 0xC922), kinds):
        w.append((at, bytes([v])))
    fw = word()
    words = [(fw * (2 * i + 1)) & 0xFFFF for i in range(0x4000)]
    for j in range(5):
        n = byte() % 6
        extra = byte()
        if j == 2 and extra < 0x40:
            n = 24 + extra % 6
        for _ in range(n):
            words[word() & 0x3FFF] = kinds[j] * 0x101
    ff = word()
    frame = bytes((ff + 7 * i) & 0xFF for i in range(0x10000))
    # The lists: taverns' at LSEG:0, stores' at 100h, temples' at 180h.
    w.append((0x1AB5, pk(0) + pk(LSEG)))
    w.append((0x1008, pk(0x100) + pk(LSEG)))
    w.append((0x1004, pk(0x180) + pk(LSEG)))
    w.append((0xD806, pk(WSEG)))
    w.append((0x9030, pk(FSEG)))
    return w, ax, words, frame


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = StoreDos(EXE, GAME)
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    snap = a.snapshot()
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
    rng = random.Random(0x1994_3EC3)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        a.mh = {}
        a.files = {}
        a.deleted = []
        writes, ax, words, frame = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(WSEG, 0), struct.pack('<16384H', *words))
        mu.mem_write(a.lin(FSEG, 0), frame)
        mu.mem_write(a.lin(LSEG, 0), bytes(0x10000))
        for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
            wreg(r, 0)
        wreg('eflags', 0x0002)
        before = [a.seg_bytes(sg) for sg in (a.ds, LSEG, FSEG, HEAP)]
        try:
            a.call_far(0x1194, 0x3EBF, ax=ax, es=a.ds)
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            # The last open file's name's segment (A90A): DGROUP, relocated.
            if struct.unpack_from('<H', ds, 0xA90A)[0] == a.ds:
                ds[0xA90A:0xA90C] = struct.pack('<H', DGROUP)
            after = [bytes(ds)] + [a.seg_bytes(sg) for sg in (LSEG, FSEG, HEAP)]
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before[0], after[0])
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[0][o]:02x} {v:02x}', file=sys.stderr)
                print('files', sorted(a.files), file=sys.stderr)
            fl = b''.join(k.encode('latin1') + b'\0' + struct.pack('<I', len(v)) + bytes(v) for k, v in sorted(a.files.items()))
            digests = [fnv(changes(x, y)) for x, y in zip(before, after)] + [fnv(fl)]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                print(f'{seed:x}: {e!r} at {reg("cs") - LOAD_SEG:04x}:{reg("ip"):04x}', file=sys.stderr)
        out.write(f'cityinfo | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
