"""Emit test vectors for the main quest's informant (src/sidequest.rs
`quest_informant`, 1194:CED7 with CF58, CFF7, D019, D105, D2F3, 49FF,
4E54, CFAF and 26F7:0C0A).

Usage: gen_informant.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/informant.rs) 5C8
(bit 4000h at times), the main quest's block (0FDD the step, 0FDE the
informant, often the person talked to, 0FE3 its dungeons' province,
0FE6 the flags, 0FE9 the time it was met, 0FED its days, 0FEE its
price), the person talked to (FD2), the time (5CC, its days since 0FE9
around 0FED), the gold (5C2, above or below the price), 1D3, the
province's data (1429, 4CCh bytes), the save's slot (1A6C), the RNG's
seed, the EMS frame's segment (9030 set to 6000h) and its bytes (the
dialogue file's strings: random bytes, NULs among them; a seed at 91F6,
which D105's roll reads with DS the frame; zeros), and the
answers: the haggle's carry and the dialog's AX.

Stubs, each a code hook at its first instruction patched to RET or RETF,
log their calls: the text (1194:A374: 1, [137A], [137E], AX), the
substitutions (2DA2:000A: 3, DI), the province's load (1194:2E18: 4,
AL), the dungeon entry's write (0000:6DDC: 5, SI, CX, [129E]), the log
(27E3:000A: 6, SI), the haggle (1194:8020: 10, EAX's words, SI, DH; it
returns the case's carry), the file's load (0000:7428: 11, SI), the
picture (1194:4B8E: 12, SI), the dialog (1194:4E88: 13, [13D3]; it
returns the case's AX) and the message (1194:4D31: 14, SI). The fonts
and 26F7:0C0A's other file calls (25FD:0844, 0000:67C4, 6D32, 6742)
return at once.

  informant | seed | result

Results: "stuck" (the original faults or runs past the cap), or the
carry and the FNV-1a digests of the changed DGROUP bytes (offset,
value), the log and the frame's 64 KB.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_AX, UC_X86_REG_EAX, UC_X86_REG_SI, UC_X86_REG_DI, UC_X86_REG_CX,
                               UC_X86_REG_DX, UC_X86_REG_EFLAGS)
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

FSEG = 0x6000


def setup(seed):
    """(DGROUP writes, the frame's bytes, the haggle's carry, the dialog's
    AX). Mirrored in tests/informant.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x05C8, pk(word() & ~0x4000 | (0x4000 if byte() < 0x10 else 0))))
    talker = word()
    w.append((0x0FD2, pk(talker)))
    who = talker & 0xFF00 | (talker & 0xFF) >> 1
    w.append((0x0FDD, bytes([byte() % 20])))
    w.append((0x0FDE, pk(who if byte() < 0xC0 else word())))
    w.append((0x0FE3, bytes([byte() % 8])))
    w.append((0x0FE6, bytes([byte() & 0xF1 | (2 if byte() < 0xD0 else 0) | (4 if byte() < 0x30 else 0)
                             | (1 if byte() < 0x80 else 0) | (8 if byte() < 0x80 else 0)])))
    met = dword()
    days = byte() % 10
    w.append((0x0FE9, pd(met)))
    w.append((0x0FED, bytes([days])))
    fee = word() % 2000
    w.append((0x0FEE, pk(fee)))
    passed = (days + byte() % 5 - 2) % 0x100
    w.append((0x05CC, pd(met + passed * 0x5A0 + word() % 0x5A0)))
    k = byte()
    w.append((0x05C2, pd(fee if k < 0x30 else fee + word() % 500 if k < 0xA0 else word() % (fee + 1))))
    w.append((0x01D3, bytes([byte()])))
    w.append((0x1429, bytes(byte() for _ in range(0x4CC))))
    w.append((0x1A6C, bytes([byte() % 100])))
    w.append((0x91F6, pd(dword())))
    w.append((0x9030, pk(FSEG)))
    # The frame: the dialogue file's strings, and at 91F6 the seed D105's
    # roll reads there (its DS is the frame); the rest zero.
    frame = bytearray(0x10000)
    # Long strings with spaces, so that lines wrap (49FF) and pages split
    # (4E54).
    if byte() < 0x80:
        frame[:0x2000] = bytes(0 if k < 0x04 else 0x20 if k < 0x30 else byte() for k in (byte() for _ in range(0x2000)))
    else:
        # Strings of lengths around the wrap's 3Ch (and twice it), a
        # space somewhere in each.
        at = 0
        while at < 0x1F00:
            n = [0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x77, 0x78, 0x79][byte() % 8]
            text = bytearray(0x41 + byte() % 26 for _ in range(n))
            text[byte() % n] = 0x20
            frame[at:at + n] = text
            at += n + 1
    frame[0x91F6:0x91FA] = pd(dword())
    frame = bytes(frame)
    cf = byte() & 1
    answer = [0, 0, 1, 2, 3][byte() % 5]
    return w, frame, cf, answer


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

    def haggle():
        eax = reg(UC_X86_REG_EAX)
        log.extend([10, eax & 0xFFFF, eax >> 16, reg(UC_X86_REG_SI), reg(UC_X86_REG_DX) >> 8])
        mu.reg_write(UC_X86_REG_EFLAGS, reg(UC_X86_REG_EFLAGS) & ~1 | answers[0])

    def dialog():
        log.extend([13, b8(0x13D3)])
        mu.reg_write(UC_X86_REG_AX, answers[1])

    for seg, off in [(0, 0x9AE6), (0, 0x9AF8), (0x25FD, 0x0844), (0, 0x67C4), (0, 0x6D32), (0, 0x6742)]:
        patch(seg, off, RETF)
    stub(0x1194, 0xA374, False, lambda: log.extend([1, a.r16(0x137A), b8(0x137E), reg(UC_X86_REG_AX)]))
    stub(0x2DA2, 0x000A, True, lambda: log.extend([3, reg(UC_X86_REG_DI)]))
    stub(0x1194, 0x2E18, False, lambda: log.extend([4, reg(UC_X86_REG_AX) & 0xFF]))
    stub(0, 0x6DDC, True, lambda: log.extend([5, reg(UC_X86_REG_SI), reg(UC_X86_REG_CX), a.r16(0x129E)]))
    stub(0x27E3, 0x000A, True, lambda: log.extend([6, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x8020, False, haggle)
    stub(0, 0x7428, True, lambda: log.extend([11, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x4B8E, False, lambda: log.extend([12, reg(UC_X86_REG_SI)]))
    stub(0x1194, 0x4E88, False, dialog)
    stub(0x1194, 0x4D31, False, lambda: log.extend([14, reg(UC_X86_REG_SI)]))
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_CED7)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, frame, answers[0], answers[1] = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(FSEG, 0), frame)
        log.clear()
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0x1194, 0xCED7)
            cf = reg(UC_X86_REG_EFLAGS) & 1
            after = a.seg_bytes(a.ds)
            lg = b''.join(struct.pack('<H', v & 0xFFFF) for v in log)
            res = f'{cf} {fnv(changes(before, after)):x} {fnv(lg):x} {fnv(a.seg_bytes(FSEG)):x}'
            if os.environ.get('DUMP') == f'{seed:x}':
                for i in range(0x10000):
                    if before[i] != after[i]:
                        print(f'{i:04x} {before[i]:02x} {after[i]:02x}', file=sys.stderr)
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'informant | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
