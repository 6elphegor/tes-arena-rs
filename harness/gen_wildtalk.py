"""Emit test vectors for two clicks of the wild (src/talk.rs): the boat
boarded (`board_boat`, 2294:0DD0) and the way told to the nearest inn,
temple or dungeon (`wild_way`, 217B:0682 with 05C2) from the original
code.

Usage: gen_wildtalk.py ACD_unpacked.EXE GAME_DIR OUT

A boat case fills from a seed (`boat_setup`, mirrored in tests/wildtalk.rs)
the boat's record (3D96: its x and y), the map's word at its square (D806
at MSEG), the level's and the scene's flags (C39A, 5CA), the hands' bank
(C8F4) and the frame's far pointer (902E, 9030 at FSEG); BOATPROL.IMG is
loaded for real (0000:7428 from GAME_DIR's GLOBAL.BSA, opened first as
the game opens it). EMS is off.

A way case (`way_setup`) fills the kind (1 an inn, 2 a temple, 0 a
dungeon), the walls' and the flat's kinds (C91A, C91B, C943), the player's
place (C3D0, C3D4) and the wild window's corner (608, 60A), the wild's
64 x 64 blocks (FSEG:F000; at times none of the kind) and the map's
plane (at times the kind's wall somewhere), and the way's words (8A52,
8A54) as before. The text (1194:A36C: 2,
[137A], AX, C3D0, C3D4, 8A52, 8A54) and the message box (1194:59C8: 1,
SI) are stubs that log.

  boat | seed | result
  wildway | seed | result

Results: "stuck", or the FNV-1a digests of the changed DGROUP bytes
(offset, value), FSEG's and MSEG's 64 KB and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_SI
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from dos import DosArena
from gen_attack import stream

MSEG = 0x6000
FSEG = 0x7000
RECORD = 0x3D96
LISTS = ([10, 17, 49, 51, 55, 57, 64, 65, 66, 67, 68, 69], [9, 16, 25, 30, 32, 37, 52, 58, 59, 60],
         [8, 18, 22, 27, 30, 34, 36, 40, 54, 61, 70])


def boat_setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes. Mirrored in
    tests/wildtalk.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    x, y = word() & 0x3FFF, word() & 0x3FFF
    w.append((None, RECORD, pk(x)))
    w.append((None, RECORD + 4, pk(y)))
    w.append((MSEG, (y >> 7) << 8 | ((x >> 7) * 2) & 0xFF, pk(word())))
    w.append((None, 0xC39A, pk(word())))
    w.append((None, 0x05CA, pk(word())))
    w.append((None, 0xC8F4, bytes([byte()])))
    w.append((None, 0x902E, pk(0x1000 * (byte() % 8))))
    w.append((None, 0x9030, pk(FSEG)))
    return w


def way_setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes and the kind.
    Mirrored in tests/wildtalk.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    kind = byte() % 3
    walls = [byte() % 0x40 for _ in range(3)]
    for off, v in zip((0xC91A, 0xC91B, 0xC943), walls):
        w.append((None, off, bytes([v])))
    w.append((None, 0xC3D0, pk(word())))
    w.append((None, 0xC3D4, pk(word())))
    w.append((None, 0x0608, pk(byte() % 0x40)))
    w.append((None, 0x060A, pk(byte() % 0x40)))
    w.append((None, 0x9030, pk(FSEG)))
    none = byte() < 0x40
    blocks = bytearray()
    for _ in range(0x1000):
        if not none and byte() < 4:
            lst = LISTS[kind]
            blocks.append(lst[byte() % len(lst)])
        else:
            blocks.append(byte() % 8)
    w.append((FSEG, 0xF000, bytes(blocks)))
    if byte() < 0x80:
        wall = (0x8000 | walls[2]) if kind == 0 else walls[kind - 1] * 0x101
        w.append((MSEG, 2 * (word() % 0x4000), pk(wall)))
    # The way's words as an earlier one left them (written as bytes when
    # the wall is on this map).
    w.append((None, 0x8A52, pk(word())))
    w.append((None, 0x8A54, pk(word())))
    return w, kind


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    log = []

    def stub(seg, off, fn):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    stub(0x1194, 0xA36C, lambda: log.extend([2, a.r16(0x137A), mu.reg_read(UC_X86_REG_AX), a.r16(0xC3D0), a.r16(0xC3D4),
                                             a.r16(0x8A52), a.r16(0x8A54)]))
    stub(0x1194, 0x59C8, lambda: log.extend([1, mu.reg_read(UC_X86_REG_SI)]))
    a.w16(0xD806, MSEG)
    a.mu.mem_write(a.lin(a.ds, 0x8FA6), b'\x00')

    def norm(b):
        """DGROUP's bytes, the segments of the message (061C) and of the
        file's name (A90A, 0000:7428's) DGROUP's."""
        b = bytearray(b)
        for at in (0x061C, 0xA90A):
            if struct.unpack_from('<H', b, at)[0] == a.ds:
                struct.pack_into('<H', b, at, DGROUP)
        return b

    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA, as the game does
    snap = a.snapshot()
    zeros = bytes(0x10000)
    total = int(os.environ.get('N', 1500))
    for name, setup, rseed in (('boat', boat_setup, 0x1994_0DD0), ('wildway', way_setup, 0x1994_0682)):
        rng = random.Random(rseed)
        for n in range(total):
            seed = rng.getrandbits(32)
            if not mine(n, total):
                continue
            a.restore(snap)
            mu.mem_write(a.lin(MSEG, 0), zeros)
            mu.mem_write(a.lin(FSEG, 0), zeros)
            r = setup(seed)
            writes, ax = (r, 0) if name == 'boat' else r
            for seg, off, data in writes:
                mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
            log.clear()
            before = norm(a.seg_bytes(a.ds))
            try:
                if name == 'boat':
                    a.call_far(0x2294, 0x0DD0, esi=RECORD)
                else:
                    a.call_far(0x217B, 0x0682, eax=ax)
                after = norm(a.seg_bytes(a.ds))
                lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
                res = f'{fnv(changes(before, after)):x} {fnv(a.seg_bytes(FSEG)):x} {fnv(a.seg_bytes(MSEG)):x} {fnv(lg):x}'
                if os.environ.get('DUMP') == f'{seed:x}':
                    for i in range(0x10000):
                        if before[i] != after[i]:
                            print(f'{i:04x} {before[i]:02x} {after[i]:02x}', file=sys.stderr)
                    print('log', [hex(v) for v in log], file=sys.stderr)
            except (Fault, AssertionError):
                res = 'stuck'
            out.write(f'{name} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
