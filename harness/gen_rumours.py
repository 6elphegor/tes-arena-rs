"""Emit test vectors for the tavern's rumours (src/tavern.rs
`tavern_rumours`, 1194:E3F7) from the original code.

Usage: gen_rumours.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/rumours.rs) what the
rumours read: the seed's word (151), the clock (5CC) and the player's x
(C3D0), whose parity picks the general rumour; the quest being told (the
34h bytes at F3E, its +4 word), the four quests at E6E (their +4 words, a
share in state 2), the font word (1036), and leftovers in the colour
(9288), the wrap width (137E) and the substitution flag (101C); and the
menu's answer.

The menu (020E), the text (A374) and the box (59CC) are stubs that log
their call: 8 menu (SI BX DI) AX; 16 text (137A, AX, DI, 137E, 101C,
B038); 10 box (SI, 9288). 0000:9AF8 (the font) runs.

  rumours | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes and of the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from dos import DosArena
from gen_attack import stream


def setup(seed):
    """(writes, menu answer). Mirrored in tests/rumours.rs, draw for
    draw."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x0151, pk(word())))
    w.append((0x05CC, bytes(byte() for _ in range(4))))
    w.append((0xC3D0, pk(word())))
    w.append((0x0F3E, bytes(byte() for _ in range(0x34))))
    q = word()
    if byte() < 0x40:
        # Nothing being told.
        q &= ~0xC0
    w.append((0x0F3E + 4, pk(q)))
    for i in range(4):
        v = word()
        if byte() < 0x40:
            v = v & ~0xC0 | 0x80
        w.append((0x0E6E + 0x34 * i + 4, pk(v)))
    w.append((0x1036, pk(word())))
    w.append((0x9288, bytes([byte()])))
    w.append((0x137E, bytes([byte()])))
    w.append((0x101C, bytes([byte()])))
    k = byte() % 5
    answer = (0xFFFF, 0, 1, 0, 1)[k]
    return w, answer


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    RETF, RET = b'\xcb', b'\xc3'
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def stub(off, far, fn):
        a.patch(0x1194, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + 0x1194, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def menu():
        log(8, reg('si'), reg('bx'), reg('di'))
        wreg('ax', state['answer'])

    def text():
        log(16, a.r16(0x137A), reg('ax'), reg('di'), a.r16(0x137E) & 0xFF, a.r16(0x101C) & 0xFF, a.r16(0xB038))

    stub(0x020E, True, menu)
    stub(0xA374, False, text)
    stub(0x59CC, False, lambda: log(10, reg('si'), a.r16(0x9288) & 0xFF))
    snap = a.snapshot()
    rng = random.Random(0x1994_E3F7)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        writes, answer = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        state.update(log=[], answer=answer)
        before = a.seg_bytes(a.ds)
        try:
            # Every case from the same registers (call_near's set-up) and
            # flags.
            mu.reg_write(UC_X86_REG_EFLAGS, 0x0002)
            a.call_near(0x1194, 0xE3F7)
            digests = [fnv(changes(before, a.seg_bytes(a.ds))),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'rumours | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
