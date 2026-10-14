"""Emit test vectors for a quest's item picked up (src/quest.rs
`quest_item_taken`, 1194:9F04) and the main quest's chest (`quest_chest`,
2294:0F33, with 1194:D140, D198, D1F1, D316 and 26F7:0C0A).

Usage: gen_mainquest.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/mainquest.rs) the
place (5E6, A46, A4C), the main quest's block (0FDD..0FEF: its step,
sometimes the one whose piece is 8Eh (`step_8e`, from the table at
78C3), its two dungeons often the place, the chest's square at 0FEE
often the chest's), the item list (0FD4), the four quests (0E6E, kind,
stage and +14h aimed at a quest item here), the clicked chest's record
(3D96: x, y), the player's level and the occupancy of their 40 item
slots (all taken at times), the RNG's seed (91F6), the province's data
(1429, 4CCh bytes) and the item buffer (13DA). The floor plane (D806)
is a zeroed segment.

Stubs, each a code hook at its first instruction patched to RET or RETF,
log their calls: the text (1194:A374: 1, [137A], [137E], AX), the
message box (1194:59CC: 2, SI), the substitutions (2DA2:000A: 3, DI),
the province's load (1194:2E18: 4, AL), the dungeon entry's write
(0000:6DDC: 5, SI, CX, [129E]), the log entry (27E3:000A: 6, SI), the
end (1194:1034: 7) and the full inventory's text (1194:42EA: 8, AX). The
fonts and 26F7:0C0A's other file calls (25FD:0844, 0000:67C4, 6D32,
6742) return at once.

  mainquest | seed | op | result

Op: 0 9F04, 1 0F33 (SI the chest's record). Results: "fault" / "hang",
or the carry and the FNV-1a digests of the changed DGROUP bytes (offset,
value), the floor plane and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_SI, UC_X86_REG_DI, UC_X86_REG_CX, UC_X86_REG_EFLAGS
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

FSEG = 0x6000
RECORD = 0x3D96


def setup(seed, step_8e):
    """(op, writes). Mirrored in tests/mainquest.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    op = byte() & 1
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w.append((0x05E6, bytes([3 if byte() < 0xD8 else byte()])))
    province = byte() % 8
    place = byte()
    w.append((0x0A46, bytes([province])))
    w.append((0x0A4C, bytes([place])))
    local = place & 0x1F
    # Columns past 7Fh (x from 4000h) wrap when doubled in 8 bits.
    col = byte() % 64 if byte() < 0xC0 else byte()
    row = byte() % 64
    w.append((RECORD, pk(col << 7 | byte() & 0x7F)))
    w.append((RECORD + 4, pk(row << 7 | byte() & 0x7F)))
    square = row << 8 | col * 2 & 0xFF
    block = bytearray(byte() for _ in range(0x13))
    block[0] = step_8e if byte() < 0x50 else byte() % 16
    k = byte() % 3
    if k == 0:
        block[5], block[6] = local, province
    elif k == 1:
        block[7], block[8] = local, province
        block[6] = (province + 1 + byte() % 7) % 8
    block[0x11:0x13] = pk(square if byte() < 0xC0 else word())
    w.append((0x0FDD, bytes(block)))
    items = [0xFF if byte() < 0x60 else byte() % 0x50 for _ in range(9)]
    w.append((0x0FD4, bytes(items)))
    for i in range(4):
        at = 0x0E6E + i * 0x34
        w.append((at, bytes(byte() for _ in range(0x34))))
        kind = 1 if byte() < 0xC0 else byte() % 4
        stage = 5 if byte() < 0x80 else byte() % 8
        w.append((at + 4, pk(word() & 0xF13F | kind << 6 | stage << 9)))
        w.append((at + 0x14, bytes([local if byte() < 0x80 else byte()])))
        w.append((at + 0x28, bytes([0 if byte() < 0x80 else byte()])))
    w.append((0x01AA, bytes([byte()])))
    full = byte() < 0x50
    for i in range(40):
        at = 0x0212 + i * 0x13 + 0x0E
        f = byte()
        f = f | 0x40 if full or byte() < 0xA0 else f & ~0x40
        w.append((at, bytes([f])))
    w.append((0x91F6, struct.pack('<I', byte() | byte() << 8 | byte() << 16 | byte() << 24)))
    w.append((0x1429, bytes(byte() for _ in range(0x4CC))))
    w.append((0x13DA, bytes(byte() for _ in range(0x13))))
    return op, w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    RETF, RET = b'\xcb', b'\xc3'
    log = []
    reg = lambda r: mu.reg_read(r)

    def patch(seg, off, code):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), code)

    def stub(seg, off, far, fn):
        patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    b8 = lambda off: mu.mem_read(a.lin(a.ds, off), 1)[0]
    for seg, off in [(0, 0x9AE6), (0, 0x9AF8), (0x25FD, 0x0844), (0, 0x67C4), (0, 0x6D32), (0, 0x6742)]:
        patch(seg, off, RETF)
    stub(0x1194, 0xA374, False, lambda: log.extend([1, a.r16(0x137A), b8(0x137E), reg(UC_X86_REG_AX)]))
    stub(0x1194, 0x59CC, False, lambda: log.extend([2, reg(UC_X86_REG_SI)]))
    stub(0x2DA2, 0x000A, True, lambda: log.extend([3, reg(UC_X86_REG_DI)]))
    stub(0x1194, 0x2E18, False, lambda: log.extend([4, reg(UC_X86_REG_AX) & 0xFF]))
    stub(0, 0x6DDC, True, lambda: log.extend([5, reg(UC_X86_REG_SI), reg(UC_X86_REG_CX), a.r16(0x129E)]))
    stub(0x27E3, 0x000A, True, lambda: log.extend([6, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x1034, True, lambda: log.extend([7]))
    stub(0x1194, 0x42EA, True, lambda: log.extend([8, reg(UC_X86_REG_AX)]))
    base_ds = a.seg_bytes(a.ds)
    step_8e = next((i for i in range(16) if base_ds[0x78C3 + i] == 0x8E), 16)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_0F33)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        mu.mem_write(a.lin(FSEG, 0), zero)
        a.w16(0xD806, FSEG)
        op, writes = setup(seed, step_8e)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                a.call_far(0x1194, 0x9F04)
            else:
                a.call_far(0x2294, 0x0F33, si=RECORD)
            cf = reg(UC_X86_REG_EFLAGS) & 1
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{cf} {fnv(changes(before, after)):x} {fnv(a.seg_bytes(FSEG)):x} {fnv(lg):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'mainquest | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
