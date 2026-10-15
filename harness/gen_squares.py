"""Emit test vectors for the walk's bob and footsteps (src/steps.rs:
`view_window` 0000:A5D1, `walk_bob` 0000:FCF6) and a level's trigger
squares (src/squares.rs: `trigger_squares` 0000:D39B, `square_lookup`
0000:06BE, `level_text` 0000:0628).

Usage: gen_squares.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/squares.rs) the op,
the player's place and the last step's, the bob's position and table, the
flags, the footstep's sound and parity, the view's window, a trigger list
with the player's square often on it, the last text and sound started,
the hooks (the sound hook CD46 as 23BA:002A, the text hook CD52 as
0000:0628, or unset), and the INF's texts: records of each kind (`+`
numbers, `^` riddles with answers, `~` texts, plain texts with lines, a
continuation after some) in a scratch segment that is also the EMS frame
(EMS off). The sound hook is a stub logging AX at FFE0, CX at FFE2 and
counting its calls at FFE4.

  squares | seed | op | result

Op: 0 `walk_bob`, 1 `view_window`, 2 `trigger_squares`, 3 `square_lookup`
(far; the result also CF, AX, BX, SI), 4 `level_text` (far). Results:
"stuck" (a fault or no return), or the FNV-1a digests of the changed
DGROUP and text segment bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_SI, UC_X86_REG_EFLAGS

TSEG = 0x7000
# mov [FFE0],ax; mov [FFE2],cx; inc word [FFE4]; retf
SOUND_STUB = bytes.fromhex('a3e0ff890ee2ffff06e4ffcb')


def setup(seed):
    """(op, DGROUP writes {off: bytes}, text bytes at TOFF, TOFF, regs, hooks).
    Mirrored in tests/squares.rs; every draw is made in order."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() % 5
    x = word()
    y = word()
    w.append((0xC3D0, pk(x)))
    w.append((0xC3D4, pk(y)))
    dx = byte() % 0xA0
    dy = byte() % 0xA0
    w.append((0xE7A2, pk(x + dx - 0x50)))
    w.append((0xE7A4, pk(y + dy - 0x50)))
    if byte() < 0xE0:
        at = byte() % 8
    else:
        at = word()
    w.append((0xE7A6, pk(at)))
    if byte() < 0x40:
        for i in range(8):
            v = byte() % 7
            w.append((0xE7A8 + 2 * i, pk(v - 3 if v != 3 else 0)))
    f = word() & ~0xC00
    if byte() < 0x30:
        f |= 0x800
    if byte() < 0x30:
        f |= 0x400
    w.append((0xC398, pk(f)))
    g = word() & ~0x4050
    if byte() < 0x40:
        g |= 0x40
    if byte() < 0x60:
        g |= 0x10
    if byte() < 0x60:
        g |= 0x4000
    w.append((0xC39A, pk(g)))
    w.append((0xC3C4, bytes([0 if byte() < 0x40 else byte()])))
    if byte() < 0xC0:
        p = byte() & 1
    else:
        p = byte()
    w.append((0xE39D, bytes([p])))
    w.append((0xC3D8, pk(word())))
    for i in range(4):
        w.append((0xC39C + 2 * i, pk(word())))
    for i in range(4):
        w.append((0xA914 + 2 * i, pk(word())))
    w.append((0xCB86, pk(word())))
    sound_hook = byte() < 0xC0
    text_hook = byte() < 0xC0
    if byte() < 0x10:
        n = byte() % 64
    else:
        n = byte() % 9
    w.append((0xC866, bytes([n])))
    ents = []
    for i in range(n):
        col = byte() % 0x40
        row = byte() % 0x40
        a = 0xFF if byte() < 0x40 else byte() % 6
        b = 0xFF if byte() < 0x40 else byte()
        ents.append((col, row))
        w.append((0xC766 + 4 * i, bytes([col, row, a, b])))
    if n > 0 and byte() < 0xC0:
        col, row = ents[byte() % n]
    else:
        col = byte() % 0x40
        row = byte() % 0x40
    w.append((0xCCA4, pk(col)))
    w.append((0xCCA6, pk(row)))
    if byte() < 0xE0:
        sq = row << 8 | (col * 2) & 0xFF
    else:
        sq = word()
    w.append((0xCB90, pk(sq)))
    for at in (0xD73D, 0xD73E):
        v = 0xFF if byte() < 0x40 else byte() % 6
        w.append((at, bytes([v])))
    w.append((0xC8FE, bytes([byte()])))
    toff = word() & 0x0FFF
    w.append((0xCB88, pk(toff)))
    t = b''
    for i in range(8):
        k = byte() % 4
        if k == 0:
            v = 0x7B if byte() < 0x40 else byte()
            t += b'+' + str(v).encode() + b'\rITEM'
        elif k == 1:
            a = byte()
            sp = byte() % 3
            b = byte()
            c1 = b'C' if byte() & 1 else b'X'
            c2 = b'C' if byte() & 1 else b'X'
            t += (b'^' + str(a).encode() + b' ' * sp + str(b).encode() + b'\rRIDDLE\r:ANS\r`' + c1
                  + b'\rONE\r`' + c2 + b'\rTWO\r')
        elif k == 2:
            t += b'~ONCE\rLINE'
        else:
            lines = byte() % 4
            t += b'TEXT' + b'\rL' * lines
            if byte() & 1:
                t += b'\r:END'
        t += b'\0'
        if byte() < 0x30:
            t += b'`CONT\0'
    regs = {}
    if op == 2:
        regs = dict(bx=row)
    elif op == 3:
        if byte() < 0xC0:
            key = row << 8 | col
        else:
            key = word()
        regs = dict(ax=key, bx=word(), si=word())
    elif op == 4:
        bx = word()
        regs = dict(bx=bx, si=word())
    return op, w, t, toff, regs, (sound_hook, text_hook)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG + 0x23BA, 0x002A), SOUND_STUB)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_D39B)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        op, w, t, toff, regs, (sh, th) = setup(seed)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(TSEG, 0), zero)
        for off, data in w:
            a.mu.mem_write(a.lin(a.ds, off), data)
        a.w16(0xCB8A, TSEG)
        a.w16(0x9030, TSEG)
        a.mu.mem_write(a.lin(a.ds, 0x8FA6), b'\0')
        a.mu.mem_write(a.lin(TSEG, toff), t)
        a.w32(0xCD46, (LOAD_SEG + 0x23BA) << 16 | 0x002A if sh else 0)
        a.w32(0xCD52, LOAD_SEG << 16 | 0x0628 if th else 0)
        before = a.seg_bytes(a.ds)
        tbefore = a.seg_bytes(TSEG)
        try:
            extra = ''
            if op == 0:
                a.call_near(0, 0xFCF6)
            elif op == 1:
                a.call_near(0, 0xA5D1)
            elif op == 2:
                a.call_near(0, 0xD39B, **regs)
            elif op == 3:
                a.call_far(0, 0x06BE, **regs)
                mu = a.mu
                cf = mu.reg_read(UC_X86_REG_EFLAGS) & 1
                extra = f' {cf} {mu.reg_read(UC_X86_REG_AX):x} {mu.reg_read(UC_X86_REG_BX):x} {mu.reg_read(UC_X86_REG_SI):x}'
            else:
                a.call_far(0, 0x0628, **regs)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x} {fnv(changes(tbefore, a.seg_bytes(TSEG))):x}' + extra
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'squares | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
