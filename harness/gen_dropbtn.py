"""Emit test vectors for the inventory's drop button (src/use_item.rs
`drop_button`, 1194:17D8) and the check before the character sheet of
where the player stands (`drop_place`, 1194:5F13) from the original code.

Usage: gen_dropbtn.py ACD_unpacked.EXE OUT

The button's cases start from gen_useitem.py's dropping set-up (`setup`
with op 5, mirrored in tests/useitem.rs) and draw from a second stream
(seed ^ 5F13h) the word 5E0 (bit 20h at times), the byte 12D0, the font
record words 1026 and 102C, the question's answer and the question's
width. Stubbed: the wait 1194:5E21 and the list's message 1194:223A
return; the question 1194:48E9 logs SI and answers (AX 0 yes, 1 no); the
pointer's restore 0000:541E returns; the text's width 0000:9CAC answers
the drawn width. Dropping (1194:18F6) runs, with gen_useitem.py's stubs.
ES is DGROUP.

The place's cases draw the player's square (CB90, even), a plane at
[D806] (7000h) with the square's byte, a plane at [CCD0] (8000h) with
the square's word, the word at 9030 (at times about C00h under it), the
17h actors' squares at 1951 (at times the player's among them), and the
words 5E0, C39A, C398, 12D0.

  dropbtn | op | seed | result

op: 0 the button, 1 the place. Results: "fault" / "hang", or the FNV-1a
digests of the changed DGROUP and FRAME bytes and of the questions' log
(the SI of each).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_SI
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter
from gen_useitem import setup as useitem_setup, stub_log, store_cast, store_si, FRAME, DATA

PLANE = 0x7000
FLOOR = 0x8000


def button(seed):
    """(DGROUP writes, answer, width) after useitem's set-up. Mirrored in
    tests/useitem.rs."""
    s = stream(seed ^ 0x5F13)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    flags = word() & ~0x20
    if byte() < 0x30:
        flags |= 0x20
    w.append((0x05E0, pk(flags)))
    lost = byte() if byte() < 0x60 else 0
    w.append((0x12D0, bytes([lost])))
    w.append((0x1026, pk(word())))
    w.append((0x102C, pk(word())))
    answer = byte() & 1
    width = byte()
    # Mostly the player's own list, at times nothing selected.
    w.append((0x129A, bytes([1 if byte() < 0xD0 else 0])))
    if byte() < 0x30:
        w.append((0x1296, b'\xff'))
    return w, answer, width


def place(seed):
    """(DGROUP writes, plane byte, floor word, square). Mirrored in
    tests/useitem.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    sq = word() & 0xFFFE
    if sq == 0xFFFE:
        sq = 0
    w = [(0xCB90, pk(sq)), (0xD806, pk(PLANE)), (0xCCD0, pk(FLOOR))]
    b = byte() if byte() < 0x40 else 0
    fl = word()
    sub = word()
    if byte() < 0x80:
        sub = (fl - 0xC00 - (byte() % 3 - 1)) & 0xFFFF
    w.append((0x9030, pk(sub)))
    squares = [word() for _ in range(0x17)]
    if byte() < 0x40:
        squares[byte() % 0x17] = sq
    w.append((0x1951, b''.join(pk(v) for v in squares)))
    w.append((0x05E0, pk(word())))
    rare = lambda: word() if byte() < 0x40 else word() & ~0x214
    w.append((0xC39A, pk(rare())))
    w.append((0xC398, pk(rare())))
    w.append((0x12D0, bytes([byte()])))
    return w, b, fl, sq


def install(a):
    """The stubs and hooks; the state they read and log to."""
    a.GAME_INT3 = a.GAME_INT3 | {(0x2433, 0x0299)}
    rel = (0x7719 - (0x23B1 + 3)) & 0xFFFF
    clash = bytes([0xFF, 0x06, 0xE6, 0xFF, 0xEB, (0x1BC9 - (0x1B78 + 6)) & 0xFF])
    for seg, off, code in [
        # gen_useitem.py's, for dropping.
        (0x29CF, 0x00FA, stub_log(0xFFE0, 0xFF00, store_cast)),
        (0x2AA1, 0x021B, stub_log(0xFFE2, 0xFF40, store_si)),
        (0x1194, 0x59C8, counter(0xFFE8, True)),
        (0x1194, 0x1BE0, counter(0xFFE4, False)),
        (0x1194, 0x1B78, clash),
        (0x1194, 0x5E21, b'\xc3'),
        (0x1194, 0x23B1, bytes([0xE8, rel & 0xFF, rel >> 8, 0xC3])),
        (0, 0x5454, b'\xcb'), (0, 0x9466, b'\xcb'), (0x3159, 0x0006, b'\xcb'),
        # The button's drawing and question.
        (0x1194, 0x223A, b'\xc3'), (0x1194, 0x48E9, b'\xc3'),
        (0, 0x541E, b'\xcb'), (0, 0x9CAC, b'\xcb'),
    ]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    state = {}

    def ask(mu, address, size, _):
        state['log'].append(mu.reg_read(UC_X86_REG_SI))
        mu.reg_write(UC_X86_REG_AX, state['answer'])

    def width(mu, address, size, _):
        mu.reg_write(UC_X86_REG_AX, state['width'])

    for (seg, off), fn in [((0x1194, 0x48E9), ask), ((0, 0x9CAC), width)]:
        at = a.lin(LOAD_SEG + seg, off)
        a.mu.hook_add(UC_HOOK_CODE, fn, None, at, at)
    return state


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    state = install(a)
    base_ds = a.seg_bytes(a.ds)
    base_data = a.seg_bytes(LOAD_SEG + DATA)
    gains = base_ds[0x4AFE:0x4B10]
    zero = bytes(0x10000)
    rng = random.Random(0x1994_17D8)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        op = n % 2
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(LOAD_SEG + DATA, 0), base_data)
        for seg in (FRAME, PLANE, FLOOR):
            a.mu.mem_write(a.lin(seg, 0), zero)
        state.update(log=[], answer=1, width=0)
        if op == 0:
            w, d, slot, extra = useitem_setup(seed, 5, gains)
            more, answer, wd = button(seed)
            w += more
            state.update(answer=answer, width=wd)
        else:
            w, b, fl, sq = place(seed)
            d = []
            a.mu.mem_write(a.lin(PLANE, sq), bytes([b]))
            a.mu.mem_write(a.lin(FLOOR, sq), struct.pack('<H', fl))
        for off, data in w:
            a.mu.mem_write(a.lin(a.ds, off), data)
        for off, data in d:
            a.mu.mem_write(a.lin(LOAD_SEG + DATA, off), data)
        regs = dict(eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, fs=a.ds, gs=a.ds)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0x17D8 if op == 0 else 0x5F13, **regs)
            after = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', after, 0x61C)[0] == LOAD_SEG + DGROUP and before[0x61C:0x61E] != after[0x61C:0x61E]:
                struct.pack_into('<H', after, 0x61C, DGROUP)
            log = b''.join(struct.pack('<H', v) for v in state['log'])
            res = f'{fnv(changes(before, bytes(after)) + changes(zero, a.seg_bytes(FRAME))):x} {fnv(log):x}'
        except (Fault, UcError):
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'dropbtn | {op} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
