"""Emit test vectors for the main quest's person in a town
(src/sidequest.rs `quest_person_talk`, 1194:D593 with D67F, D863, D8DE,
D8B7 and 26F7:0C0A).

Usage: gen_questperson.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/questperson.rs) the
main quest's stage (0F77, 0..8) and flags (0F78..0F7F), the province
(A46, often the stage's from 80ECh) and the place's name (A22, often the
stage's town from 80F4h), the area (5E6), outdoors (C398 bit 4), the
building (D48), the level, the province's data (1429, 4CCh bytes), the
save's slot, the RNG's seed, the question's text as A374 would leave it
at 9622 (lines, line feeds and returns, its NUL), and the answers: the
menu's pick and whether "%tem" becomes the stage's temple's name.

Stubs, each a code hook at its first instruction patched to RET or RETF,
log their calls: the text (1194:A374: 1, [137A], [137E], AX), the
message box (1194:59CC: 2, SI), the substitutions (2DA2:000A: 3, DI; it
writes the stage's temple's name there when the case says so), the
province's load (1194:2E18: 4, AL), the dungeon entry's write
(0000:6DDC: 5, SI, CX, [129E]), the log (27E3:000A: 6, SI) and the menu
(1194:020E: 7, SI, BX, DI; it returns the case's pick). The fonts and
26F7:0C0A's other file calls return at once. 26F7:0C0A records its
caller's CX and SI at 1296 and 12A0, left here by the text routine: those
words are left out of the comparison.

  questperson | seed | result

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


def nth_string(ds, at, n):
    """The n-th NUL-ended string from DS:at (1194:5585), its bytes."""
    for _ in range(n):
        at = ds.index(0, at) + 1
    return ds[at:ds.index(0, at)]


def setup(seed, ds):
    """(DGROUP writes, the menu's pick, the temple's name or None).
    Mirrored in tests/questperson.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    stage = byte() % 9
    w.append((0x0F77, bytes([stage])))
    w.append((0x0F78, bytes([byte() & 1, byte() & 1, byte(), byte() & 1, byte() & 1, byte() & 1, byte(), byte()])))
    w.append((0x0A46, bytes([ds[0x80EC + stage] if byte() < 0xC0 else byte() % 9])))
    town = nth_string(ds, 0x80F4, stage)
    if byte() < 0xC0:
        name = bytes(c ^ 0x20 if 0x41 <= c & 0xDF <= 0x5A and byte() < 0x40 else c for c in town)
    else:
        name = bytes(0x41 + byte() % 26 for _ in range(byte() % 8))
    w.append((0x0A22, name + b'\0'))
    w.append((0x05E6, bytes([4 if byte() < 0x80 else byte()])))
    w.append((0xC398, pk(word() & ~4 | (4 if byte() < 0x30 else 0))))
    w.append((0x0D48, bytes([[1, 5, 7, 4, byte()][byte() % 5]])))
    w.append((0x01AA, bytes([byte() if byte() < 0x10 else byte() % 20])))
    w.append((0x1429, bytes(byte() for _ in range(0x4CC))))
    w.append((0x1A6C, bytes([byte() % 100])))
    w.append((0x91F6, struct.pack('<I', dword())))
    text = bytearray()
    for _ in range(byte() % 120):
        k = byte()
        text.append(0x0D if k < 0x18 else 0x0A if k < 0x30 else 0x20 + byte() % 0x5F)
    w.append((0x9622, bytes(text) + b'\0'))
    pick = [0, 0, 1, 0xFFFF][byte() % 4]
    temple = nth_string(ds, 0x822B, stage) if byte() < 0xA0 else None
    return w, pick, temple


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    RETF, RET = b'\xcb', b'\xc3'
    log = []
    answers = [0, None]
    reg = lambda r: mu.reg_read(r)

    def patch(seg, off, code):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), code)

    def stub(seg, off, far, fn):
        patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    b8 = lambda off: mu.mem_read(a.lin(a.ds, off), 1)[0]

    def substitute():
        di = reg(UC_X86_REG_DI)
        log.extend([3, di])
        if answers[1] is not None:
            mu.mem_write(a.lin(a.ds, di), answers[1] + b'\0')

    def menu():
        log.extend([7, reg(UC_X86_REG_SI), reg(UC_X86_REG_BX), reg(UC_X86_REG_DI)])
        mu.reg_write(UC_X86_REG_AX, answers[0])

    for seg, off in [(0, 0x9AE6), (0, 0x9AF8), (0x25FD, 0x0844), (0, 0x67C4), (0, 0x6D32), (0, 0x6742)]:
        patch(seg, off, RETF)
    stub(0x1194, 0xA374, False, lambda: log.extend([1, a.r16(0x137A), b8(0x137E), reg(UC_X86_REG_AX)]))
    stub(0x1194, 0x59CC, False, lambda: log.extend([2, reg(UC_X86_REG_SI)]))
    stub(0x2DA2, 0x000A, True, substitute)
    stub(0x1194, 0x2E18, False, lambda: log.extend([4, reg(UC_X86_REG_AX) & 0xFF]))
    stub(0, 0x6DDC, True, lambda: log.extend([5, reg(UC_X86_REG_SI), reg(UC_X86_REG_CX), a.r16(0x129E)]))
    stub(0x27E3, 0x000A, True, lambda: log.extend([6, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x020E, True, menu)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_D593)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, answers[0], answers[1] = setup(seed, base_ds)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0xD593)
            after = bytearray(a.seg_bytes(a.ds))
            for at in (0x1296, 0x1297, 0x12A0, 0x12A1):
                after[at] = before[at]
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{fnv(changes(before, after)):x} {fnv(lg):x}'
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'questperson | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
