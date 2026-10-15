"""Emit test vectors for a quest's ambush (src/sidequest.rs
`quest_ambush`, 1194:86F2 with 9F8A, 8AF0, 89A7 and 89D4's row FFFFh).

Usage: gen_ambush.py ACD_unpacked.EXE GAME_DIR OUT

Set up as gen_encounter.py (the game's heap, DOS handle 5 taken, the
fight's graphics and music counted at DGROUP FFE0..FFE4). The text
(1194:A374) and the fonts are stubbed: the text logs its calls (1,
[137A], [137E], AX).

A case fills from a seed (`setup`, mirrored in tests/ambush.rs) the
player's square (in a building's at times) and the walls around it, the
hour, the area (5E6, 1013, D48, 191D), the fight and scene flags, the
level, the creature slots and the first creature's type, the four
quests (kinds, stages, +22h the ambush's row), the countdown (1404, often
1), the list of daytime types at 681Dh as the image has it, and the
RNG's seed (half the time chosen, through the LCG's inverse multiplier
3F154FA5h, so that the d100 comes out under 16).

  ambush | seed | result

Results: "stuck" (the original faults or runs past the cap), or the
FNV-1a digests of the changed DGROUP bytes (offset, value), the creature
segment's first 32 KB, the heap's 320 KB and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from dos import DosArena
from gen_attack import stream
from gen_frame import counter
from gen_guards import WSEG, FSEG, CSEG, FRAME


def setup(seed):
    """(segment or None, offset, bytes) writes. Mirrored in
    tests/ambush.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    px, py = 0x1000 + word() % 0x2000, 0x1000 + word() % 0x2000
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    sq = (py >> 7 << 8) + (px >> 7 << 1)
    w.append((None, 0xCB90, pk(sq)))
    w.append((None, 0x05D6, pk(byte() % 24)))
    w.append((None, 0x05E6, pb(byte() % 6)))
    w.append((None, 0x1013, pb(1 if byte() < 0x30 else 0)))
    w.append((None, 0x0D48, pb(byte() % 12)))
    w.append((None, 0x191D, pb(byte() & 1)))
    w.append((None, 0x05CA, pk(0x200 if byte() < 0x08 else 0x40 if byte() < 0x40 else 0)))
    w.append((None, 0x0FF7, pk(1 if byte() < 0x08 else 0)))
    w.append((None, 0x0FFB, pk(1 if byte() < 0x08 else 0)))
    w.append((None, 0x0FF9, pk(0 if byte() < 0xC0 else 1 + byte() % 3)))
    w.append((None, 0x01AA, pb(byte() % 30)))
    w.append((None, 0x1404, pb(1 if byte() < 0xC0 else byte())))
    for i in range(8):
        si = 0x0C57 + 0x1C * i
        w.append((None, si + 0xF, pk(word() & 0x3FFF | (0x4000 if byte() < 0xC0 else 0))))
    for q in range(4):
        e = 0x0E6E + 0x34 * q
        kind = 1 if q == 0 and byte() < 0xA0 else [0, 1, 1, 2, 3][byte() % 5]
        stage = 6 if byte() < 0x30 else byte() % 8
        w.append((None, e + 4, pk(kind << 6 | stage << 9)))
        w.append((None, e + 0x22, pk(byte() % 8)))
        w.append((None, e + 0x2F, pb(0x18 + byte() % 8 if byte() < 0x80 else byte() % 0x18)))
        w.append((None, e + 0x30, pb(0x18 + byte() % 8 if byte() < 0x80 else byte() % 0x18)))
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            at = (((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)
            w.append((WSEG, at & 0xFFFF, pk(0 if byte() < 0xC0 else word() & 0x7FFF)))
    w.append((WSEG, sq, pk(0 if byte() < 0xE0 else 0x0102 * (1 + byte() % 0x40))))
    w.append((CSEG, 4, pb(byte() % 0x30)))
    w.append((CSEG, 5, pb(byte() % 8)))
    w.append((CSEG, 0x40D, pk(word() & 0x1000)))
    seed4 = bytes(byte() for _ in range(4))
    if byte() < 0x80:
        # The d100 (86F2's first roll) made small: the seed whose next
        # step's high word is under the chance, through the LCG's inverse.
        h = byte() % 16 + 100 * (word() % 655)
        nxt = h << 16 | word()
        seed4 = struct.pack('<I', nxt * 0x3F154FA5 & 0xFFFFFFFF)
    w.append((None, 0x91F6, seed4))
    return w


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    log = []
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True))]:
        mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    for seg, off in [(0, 0x9AE6), (0, 0x9AF8)]:
        mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    text = a.lin(LOAD_SEG + 0x1194, 0xA374)
    mu.mem_write(text, b'\xc3')
    b8 = lambda off: mu.mem_read(a.lin(a.ds, off), 1)[0]
    mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: log.extend(
        [1, a.r16(0x137A), b8(0x137E), mu.reg_read(UC_X86_REG_AX)]), None, text, text)
    for o, v in [(0xD806, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0x9030, FRAME), (0xCCCE, FRAME + 0xC00),
                 (0x91F0, 0), (0x05C8, 0), (0xC38E, 0xC0), (0xC357, 0x80), (0xC359, 0x80)]:
        a.w16(o, v)
    a.init_heap()
    a.handles[5] = [b'', 0]
    base_ds = a.seg_bytes(a.ds)
    base_heap = bytes(mu.mem_read(0xA0000, 0x50000))
    zero = bytes(0x10000)
    rng = random.Random(0x1994_86F2)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        mu.mem_write(0xA0000, base_heap)
        for seg in (WSEG, FSEG, CSEG):
            mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed):
            mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x86F2)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = (f'{fnv(changes(before, after)):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} '
                   f'{fnv(bytes(mu.mem_read(0xA0000, 0x50000))):x} {fnv(lg):x}')
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'ambush | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
