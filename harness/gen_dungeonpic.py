"""Emit test vectors for a main quest's dungeon shown on arrival
(src/dungeonpic.rs `dungeon_picture`, 292B:0362).

Usage: gen_dungeonpic.py ACD_unpacked.EXE GAME_DIR OUT [N]  (SHARD=k/K: par.py)

Runs under the DOS file layer (harness/dos.py) with the user's files and
EMS on with the frame at FRAME ([902E] 0), so that the picture and
DUNGEON.TXT load as they do. The drawing is stubbed, each stub logging:
0000:5364 (the line: SI, AX, BX), 9466 (shown), 5E3E (its far pointer's
segment, offset and steps: the fades 1194:55B2 makes to DS:929E logged
as "fade") and 9426 and 5828 bare. 1194:564D is stubbed by the case's
clicks: each puts its x, y at 8F92, 8F94 and clears 05C8 bit 1, as
564D does; past the last, the button's middle.

A case (seed = its number, `setup`, mirrored in tests/dungeonpic.rs)
sets AX (mostly a province, 0..8; at times any byte or word), the words
102E, 1030 (a bank below 20h), A902, 05C8 and the clicks: up to three
outside the button (or on its edges' outsides), then one in it.

  dungeonpic | seed | result

result: "fault", or the FNV-1a digests of the changed DGROUP bytes and
of the stubs' log.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_SI, UC_X86_REG_SP, UC_X86_REG_SS
from dos import DosArena
from emu import Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

FRAME = 0x8800
RETF = b'\xcb'


def setup(seed):
    """AX, the DGROUP words and the clicks. Mirrored in
    tests/dungeonpic.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    k = byte()
    ax = byte() % 9 if k < 0xC0 else byte() if k < 0xF0 else word()
    words = [(0x102E, word()), (0x1030, byte() % 0x20), (0xA902, word()), (0x05C8, word())]
    clicks = []
    for _ in range(byte() % 4):
        edge = byte() % 5
        x, y = 0x110 + byte() % 0x2B, 0xB7 + byte() % 0x0D
        if edge == 0:
            x = 0x10F
        elif edge == 1:
            x = 0x13B
        elif edge == 2:
            y = 0xB6
        elif edge == 3:
            y = 0xC4
        else:
            x, y = word() % 0x140, word() % 0xC8
            if 0x110 <= x <= 0x13A and 0xB7 <= y <= 0xC3:
                x = 0
        clicks.append((x, y))
    clicks.append((0x110 + byte() % 0x2B, 0xB7 + byte() % 0x0D))
    return ax, words, clicks


def main():
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    total = int(sys.argv[4]) if len(sys.argv) > 4 else 2000
    a = DosArena(exe, game)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    a.w16(0x902E, 0)
    log = []
    clicks = []
    reg = lambda r: a.mu.reg_read(r)

    def arg(i):
        return struct.unpack('<H', a.mu.mem_read(a.lin(reg(UC_X86_REG_SS), reg(UC_X86_REG_SP) + 4 + 2 * i), 2))[0]

    def fade(*_):
        off, seg, steps = arg(0), arg(1), arg(2)
        if seg == a.ds and off == 0x929E and steps == 8:
            log.append(b'fade')
        else:
            log.append(b'fadein' + struct.pack('<3H', seg, off, steps))

    def click(*_):
        x, y = clicks.pop(0) if clicks else (0x125, 0xBD)
        a.w16(0x8F92, x)
        a.w16(0x8F94, y)
        # 564D's own state: 5C8 bit 1 (Esc) cleared however it returns.
        a.w16(0x05C8, a.r16(0x05C8) & ~2 & 0xFFFF)
        log.append(b'click')
    hooks = {
        (0, 0x5364): lambda *_: log.append(b'line' + struct.pack('<3H', reg(UC_X86_REG_SI), reg(UC_X86_REG_AX), reg(UC_X86_REG_BX))),
        (0, 0x9466): lambda *_: log.append(b'shown'),
        (0, 0x5E3E): fade,
        (0x1194, 0x564D): click,
        (0, 0x9426): None,
        (0, 0x5828): None,
    }
    for (seg, off), fn in hooks.items():
        a.patch(seg, off, RETF)
        if fn:
            lin = a.lin(LOAD_SEG + seg, off)
            a.mu.hook_add(UC_HOOK_CODE, fn, None, lin, lin)
    base = a.snapshot()
    with open(out, 'w') as f:
        for n in range(total):
            ax, words, cl = setup(n)
            if not mine(n, total):
                continue
            a.restore(base)
            for at, v in words:
                a.w16(at, v)
            log.clear()
            clicks[:] = cl
            ds0 = a.seg_bytes(a.ds)
            try:
                a.call_far(0x292B, 0x0362, eax=ax)
                # The last load's segment, kept at A90A: DGROUP's, the
                # image's as the port's (gen_text.py).
                if a.r16(0xA90A) == a.ds:
                    a.w16(0xA90A, a.ds - LOAD_SEG)
                ds1 = a.seg_bytes(a.ds)
                r = '%x %x' % (fnv(changes(ds0, ds1)), fnv(b'.'.join(log)))
                if os.environ.get('PIC_DUMP') == f'{n:x}':
                    for o in range(0x10000):
                        if ds0[o] != ds1[o]:
                            print(f'{o:04x} {ds0[o]:02x} {ds1[o]:02x}', file=sys.stderr)
            except (Fault, UcError, AssertionError):
                r = 'fault'
            f.write(f'dungeonpic | {n:x} | {r}\n')


if __name__ == '__main__':
    main()
