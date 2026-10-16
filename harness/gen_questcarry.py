"""Emit test vectors for the frame's hold on a quest's message
(src/sidequest.rs `quest_carry`, 0000:4D00 with 1194:9F8A) and the main
quest's check on entering a temple or a guild (`main_quest_check`,
1194:D961) from the original code.

Usage: gen_questcarry.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/questcarry.rs) the
four quests' flag words (0E6E + 34h each, +4: the kind in bits 6-7, the
stage in 9-11), the place [5E38] and the 40h places at C954 (from a small
range, so that it is often among them).

  questcarry | seed | carry

A check's case (`check_setup`, mirrored too) fills the main quest's flag
(0F76), its stage (0F77), the stages' buildings (8223h) and provinces
(80ECh) and the province (A46); the person's talk (1194:D593) is a stub.

  questcheck | seed | carry ran

The two runs' cases are in turn in one file, so the cases aren't sharded.
"""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EFLAGS
from emu import Arena, LOAD_SEG
from gen_attack import stream


def setup(seed):
    """(offset, word) and (offset, byte) writes. Mirrored in
    tests/questcarry.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    words, bytes_ = [], []
    for k in range(4):
        kind = (0, 1, 2, 3, 1)[byte() % 5]
        k3 = byte() % 3
        stage = 4 if k3 == 0 else 6 if k3 == 1 else byte() % 8
        words.append((0x0E6E + 0x34 * k + 4, kind << 6 | stage << 9 | (byte() << 8 | byte()) & 0xF13F))
    bytes_.append((0x5E38, byte() % 0x60))
    for i in range(0x40):
        bytes_.append((0xC954 + i, byte() % 0x60))
    return words, bytes_


def check_setup(seed):
    """(offset, byte) writes. Mirrored in tests/questcarry.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    w = [(0x0F76, byte() & 1), (0x0F77, byte() % 10), (0x0A46, byte() % 9)]
    for i in range(10):
        w.append((0x8223 + i, (0, 5, 7, 1)[byte() % 4]))
        w.append((0x80EC + i, byte() % 9))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    rng = random.Random(0x1994_4D00)
    total = int(os.environ.get('N', 3000))
    for _ in range(total):
        seed = rng.getrandbits(32)
        words, bytes_ = setup(seed)
        for off, v in words:
            a.w16(off, v)
        for off, v in bytes_:
            a.mu.mem_write(a.lin(a.ds, off), bytes([v]))
        a.call_near(0, 0x4D00)
        out.write(f'questcarry | {seed:x} | {a.mu.reg_read(UC_X86_REG_EFLAGS) & 1}\n')
    ran = []
    a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, 0xD593), b'\xcb')
    lin = a.lin(LOAD_SEG + 0x1194, 0xD593)
    a.mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: ran.append(1), None, lin, lin)
    rng = random.Random(0x1994_D961)
    for _ in range(total):
        seed = rng.getrandbits(32)
        for off, v in check_setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), bytes([v]))
        ran.clear()
        a.call_near(0x1194, 0xD961)
        out.write(f'questcheck | {seed:x} | {a.mu.reg_read(UC_X86_REG_EFLAGS) & 1} {len(ran)}\n')
    out.close()


if __name__ == '__main__':
    main()
