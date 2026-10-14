"""Emit test vectors for a quest dungeon's last level reached (src/quest.rs
`quest_level`, 1194:9FAC with its candidates' pick A0FC, the flat A160
and the free square nearby A180).

Usage: gen_questlevel.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/questlevel.rs) the
flags (5C8, 5CA, 5E0), the area's kind (5E6, mostly 3), the level (BB0A)
and the levels' count (C355, mostly the next), the place (A46, A4C), the
map's size (C357, C359), the candidates' list (C6E5, its count at C765:
x and y bytes, some 0, FFh or outside the map), the floor plane (the
segment at D806: some squares taken), the four quests (0E6E: kind, goal
bit, type 7, stage 4..6, province and place mostly this one's; their
seeds and the fields the name's seed adds up) and the RNG's seed (91F6).

The name (1194:080B) is a stub that logs AX, EBX's halves and DI.

  questlevel | seed | result

Results: "fault" / "hang", or the carry, AX and BX, and the FNV-1a
digests of the changed DGROUP bytes (offset, value), the floor plane and
the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_EBX, UC_X86_REG_DI, UC_X86_REG_EFLAGS
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

FSEG = 0x6000


def setup(seed):
    """(writes, floor writes). Mirrored in tests/questlevel.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x05C8, pk(word() & ~0x4000 if byte() < 0xE0 else word())))
    w.append((0x05CA, pk(word())))
    w.append((0x05E0, pk(word())))
    w.append((0x05E6, bytes([3 if byte() < 0xE0 else byte()])))
    level = byte() % 8
    w.append((0xBB0A, bytes([level])))
    w.append((0xC355, pk(level + 1 if byte() < 0xE0 else byte() % 9)))
    province = byte() % 8
    place = byte()
    w.append((0x0A46, bytes([province])))
    w.append((0x0A4C, bytes([place])))
    width, depth = 8 + byte() % 57, 8 + byte() % 57
    w.append((0xC357, pk(width)))
    w.append((0xC359, pk(depth)))
    count = byte() % 64 + (0 if byte() < 0x08 else 1)
    w.append((0xC765, bytes([count])))
    cands = []
    for _ in range(64):
        k = byte() % 16
        if k == 0:
            cands.append(0xFFFF)
        elif k == 1:
            cands.append(byte() % width)
        elif k == 2:
            cands.append(word())
        elif k == 3:
            # Exactly on the map's edge.
            cands.append((1 + byte() % (depth - 1)) << 8 | width)
        else:
            cands.append((1 + byte() % (depth - 1)) << 8 | 1 + byte() % (width - 1))
    w.append((0xC6E5, b''.join(pk(c) for c in cands)))
    # The flats' kinds for quests and their people (level data).
    w.append((0xC934, bytes(byte() for _ in range(0x30))))
    floor = []
    for _ in range(byte() % 64):
        floor.append(((byte() % depth) << 8 | (byte() % width) * 2, word() | 1))
    for i in range(4):
        at = 0x0E6E + i * 0x34
        w.append((at, bytes(byte() for _ in range(0x34))))
        kind = 1 if byte() < 0xD0 else byte() % 4
        stage = [4, 5, 6][byte() % 3] if byte() < 0xD0 else byte() % 8
        ty = 7 if byte() < 0xD0 else byte() % 16
        flags = word() & 0x011F | (0x20 if byte() < 0x20 else 0) | kind << 6 | stage << 9 | ty << 12
        w.append((at + 4, pk(flags)))
        w.append((at + 0x25, bytes([(province if byte() < 0xD0 else byte() % 8) << 5 | byte() % 32])))
        w.append((at + 0x14, bytes([place & 0x1F if byte() < 0xD0 else byte()])))
        w.append((at + 0x28, bytes([byte() % 4])))
        w.append((at + 0x2F, bytes([byte() % 16])))
        w.append((at + 0x30, bytes([byte() % 16])))
    w.append((0x91F6, struct.pack('<I', dword())))
    return w, floor


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    log = []
    lin = a.lin(LOAD_SEG + 0x1194, 0x080B)
    mu.mem_write(lin, b'\xc3')
    ebx = lambda: mu.reg_read(UC_X86_REG_EBX)
    mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: log.extend(
        [mu.reg_read(UC_X86_REG_AX), ebx() & 0xFFFF, ebx() >> 16, mu.reg_read(UC_X86_REG_DI)]), None, lin, lin)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_9FAC)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        mu.mem_write(a.lin(FSEG, 0), zero)
        a.w16(0xD806, FSEG)
        writes, floor = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        for sq, v in floor:
            mu.mem_write(a.lin(FSEG, sq), struct.pack('<H', v))
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x9FAC, cx=1)
            cf = mu.reg_read(UC_X86_REG_EFLAGS) & 1
            # AX (the name) and BX (the person's flat) only with the carry;
            # otherwise they are left over and the callers ignore them.
            ax, bx = (mu.reg_read(UC_X86_REG_AX), mu.reg_read(UC_X86_REG_BX)) if cf else (0, 0)
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{cf} {ax:x} {bx:x} {fnv(changes(before, after)):x} {fnv(a.seg_bytes(FSEG)):x} {fnv(lg):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'questlevel | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
