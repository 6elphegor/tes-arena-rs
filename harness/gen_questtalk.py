"""Emit test vectors for a talk's quests (src/sidequest.rs `quests_talk`,
1194:8F1E with 8F50, 8F7B, 8F99, 9041, 90B9, 9CD8 and 26F7:0C0A).

Usage: gen_questtalk.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/questtalk.rs) the
talk (5C8 bits 4000h, 20h; the talk's kind 137F, the area 5E6, the
building D48, the place A4C, A46, A48, A4A, the player's square C3D0,
C3D4, the person FD2), the eight records (their kinds, stages, flags and
places, aimed at the place and the person at times), the player's
level, gold, experience and 5CA, 8A56, the quest items (0FD4) and D39,
the province's places (1445) and data (1429, 4CCh bytes, with its
dungeons), the save's slot (1A6C), the journey's days (1927), the RNG's
seed, and the answers: the offer's (A374 mode 1) and the palace's
yes-or-no (020E).

Stubs, each a code hook at its first instruction patched to RET or RETF,
log their calls: the text (1194:A374: 1, [137A], [137E], AX, the
player's square C3D0, C3D4 and the RNG's seed, which its substitutions
would read; it returns the case's answer), the message box (1194:59CC: 2, SI), the
substitutions (2DA2:000A: 3, DI), the province's load (1194:2E18: 4, AL),
the dungeon entry's write (0000:6DDC: 5, SI, CX, [129E]), the log
(27E3:000A: 6, SI), the menu (1194:020E: 7, SI, BX, DI; it returns the
case's pick) and the journey (1194:2E69: 9). The fonts and 26F7:0C0A's
other file calls (25FD:0844, 0000:67C4, 6D32, 6742) return at once.

  questtalk | seed | result

Results: "stuck" (the original faults or runs past the cap), or the
FNV-1a digests of the changed DGROUP bytes (offset, value) and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_SI, UC_X86_REG_DI, UC_X86_REG_CX
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream


def setup(seed):
    """(DGROUP writes, the offer's answer, the menu's). Mirrored in
    tests/questtalk.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x05C8, pk(word() & ~0x4020 | (0x4000 if byte() < 0x10 else 0) | (0x20 if byte() < 0x40 else 0))))
    w.append((0x137F, bytes([4 if byte() < 0x60 else byte()])))
    w.append((0x05E6, bytes([4 if byte() < 0xA0 else byte()])))
    building = 1 if byte() < 0x60 else byte()
    w.append((0x0D48, bytes([building])))
    here = byte()
    w.append((0x0A4C, bytes([here])))
    w.append((0x0A46, bytes([byte() % 8])))
    for at in (0x0A48, 0x0A4A, 0xC3D0, 0xC3D4):
        w.append((at, pk(word())))
    talker = word()
    w.append((0x0FD2, pk(talker)))
    who = talker & 0xFF00 | (talker & 0xFF) >> 1
    for base, size in ((0x0DCA, 0x29), (0x0E6E, 0x34)):
        for i in range(4):
            rec = bytearray(byte() for _ in range(size))
            kind = [0, 1, 1, 2, 2, 3][byte() % 6]
            stage = byte() % 8
            k = word() & ~0x0EC0 | kind << 6 | stage << 9
            rec[4:6] = pk(k)
            if byte() < 0xA0:
                rec[0x24] = here
            person = 0xAAAA if building == 1 and byte() < 0x80 else who
            if byte() < 0xA0:
                rec[0x10:0x12] = pk(person)
            if byte() < 0xA0:
                rec[0x14:0x16] = pk(person)
            w.append((base + i * size, bytes(rec)))
    w.append((0x01AA, bytes([byte() if byte() < 0x10 else byte() % 40])))
    w.append((0x05C2, pd(dword())))
    w.append((0x05AD, pd(dword())))
    w.append((0x05CA, pk(word())))
    w.append((0x8A56, bytes([byte()])))
    w.append((0x0AB0, pk(word())))
    w.append((0x0FD4, bytes(0xFF if byte() < 0x80 else byte() for _ in range(9))))
    w.append((0x0D39, bytes([byte()])))
    w.append((0x1445, bytes(byte() for _ in range(16 * 0x19))))
    w.append((0x1429, bytes(byte() for _ in range(0x4CC))))
    w.append((0x1A6C, bytes([byte() % 100])))
    w.append((0x1927, pk(byte() % 40)))
    w.append((0x91F6, pd(dword())))
    answer = [0, 0, 1, 2][byte() % 4]
    pick = [0, 0, 1, 0xFFFF][byte() % 4]
    return w, answer, pick


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    RETF, RET = b'\xcb', b'\xc3'
    log = []
    answers = [0, 0]
    reg = lambda r: mu.reg_read(r)

    def patch(seg, off, code):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), code)

    def stub(seg, off, far, fn):
        patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    b8 = lambda off: mu.mem_read(a.lin(a.ds, off), 1)[0]

    def text():
        seed = a.r16(0x91F6), a.r16(0x91F8)
        log.extend([1, a.r16(0x137A), b8(0x137E), reg(UC_X86_REG_AX), a.r16(0xC3D0), a.r16(0xC3D4), *seed])
        mu.reg_write(UC_X86_REG_AX, answers[0])

    def menu():
        log.extend([7, reg(UC_X86_REG_SI), reg(UC_X86_REG_BX), reg(UC_X86_REG_DI)])
        mu.reg_write(UC_X86_REG_AX, answers[1])

    for seg, off in [(0, 0x9AE6), (0, 0x9AF8), (0x25FD, 0x0844), (0, 0x67C4), (0, 0x6D32), (0, 0x6742)]:
        patch(seg, off, RETF)
    stub(0x1194, 0xA374, False, text)
    stub(0x1194, 0x59CC, False, lambda: log.extend([2, reg(UC_X86_REG_SI)]))
    stub(0x2DA2, 0x000A, True, lambda: log.extend([3, reg(UC_X86_REG_DI)]))
    stub(0x1194, 0x2E18, False, lambda: log.extend([4, reg(UC_X86_REG_AX) & 0xFF]))
    stub(0, 0x6DDC, True, lambda: log.extend([5, reg(UC_X86_REG_SI), reg(UC_X86_REG_CX), a.r16(0x129E)]))
    stub(0x27E3, 0x000A, True, lambda: log.extend([6, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x2E69, True, lambda: log.extend([9]))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_8F1E)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, answers[0], answers[1] = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0x1194, 0x8F1E)
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{fnv(changes(before, after)):x} {fnv(lg):x}'
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'questtalk | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
