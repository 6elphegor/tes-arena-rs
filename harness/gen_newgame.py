"""Emit test vectors for a new game's character creation (src/newgame.rs)
from the original code.

Usage: gen_newgame.py ACD_unpacked.EXE GAME_DIR OUT

Each case is one step of 1194:2498 (or of the class quiz, 222C:0000),
run on a state filled from a seed (`setup`, mirrored in
tests/newgame.rs):

  op 0  reset    1194:24A6..2512: the new game's state cleared
  op 1  pick     222C:057F: the next question, 1..40, not asked yet
  op 2  find     222C:0525: question AX found in QUESTION.TXT (at QSEG)
  op 3  answers  222C:02D4..02DE: the question's answers parsed (05C9)
  op 4  lookup   222C:005B: the class for the answers' counts (CLASSES.DAT)
  op 5  class    1194:26DC: the class's starting weapon, its class kind
  op 6  roll     1194:2AC7: the attributes, gold, hit points, points rolled
  op 7  adjust   1194:14C5 / 138E: attributes picked and raised or lowered
  op 8  finish   1194:2D28 then 2BAE: the hit points and base attributes
  op 9  face     1194:2C72..2C95: the face picked, FD4
  op 10 tail     1194:257B..25B7: the end of creation
  op 11 setup    21EE:016C: the game's set-up (the date, 0000:13FC; the
                 weather, 1194:2FB3; the tables and allocations)
  op 12 start    0000:0C66..0C95: the start dungeon chosen, before its run

What the port leaves to hooks is a stub: a code hook at its first
instruction, patched to RET or RETF, logs the call (an id and registers):

  1 message box (SI)  1194:59CC     2 stats screen        1194:0DC7
  3 palette           1194:1695     4 creation's end      0000:5D70
  5 game screen       2294:11F2     6 resources loaded    2294:01F1
  7 hands             0000:3C4C     8 level's flats       21EE:0134
  9 spells loaded     29CF:0691

The screens, images, fonts, waits and music return at once (listed in
`main`). EMS is off (the frame, 9030, is FRAME); QUESTION.TXT is put at
QSEG:0 for ops 2 and 3, as 222C:0185 loads it into its bank.

  newgame | seed | op | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes, of QSEG's changed bytes, of the log, and the registers the step
returns (AX for lookup, DI for find).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import *
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from dos import DosArena
from gen_attack import stream

QSEG = 0x6000
FRAME = 0x7000
OPS = 13


def setup(seed, op):
    """(writes, regs, clicks). Mirrored in tests/newgame.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pk4 = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = [(0x9030, pk(FRAME)), (0x91F6, pk4(word() | word() << 16))]
    regs = {}
    clicks = []
    if op == 0:
        for at, n in ((0x01A4, 0x41E), (0x0F76, 0xB), (0x0E6E, 0xA4), (0x0DCA, 0xD0), (0x0FD4, 0xA),
                      (0x0652, 0xC), (0x0FDD, 0x13)):
            w.append((at, bytes(byte() for _ in range(n))))
        for at in (0x12BA, 0x05E0, 0x05C8, 0x0AB2):
            w.append((at, pk(word())))
        w.append((0x1299, bytes([byte()])))
        w.append((0x05CC, pk4(word() | word() << 16)))
    elif op == 1:
        k = byte() % 11
        w.append((0x40C7, bytes([k])))
        w.append((0x40CB, bytes(1 + byte() % 40 if byte() < 0xE0 else byte() for _ in range(10))))
    elif op == 2:
        regs['eax'] = 1 + byte() % 40 if byte() < 0xF0 else byte() % 48
        w.append((0x40E8, pk(0) + pk(QSEG)))
    elif op == 3:
        regs['eax'] = 1 + byte() % 40
        w.append((0x40E8, pk(0) + pk(QSEG)))
    elif op == 4:
        if byte() < 0xC0:
            a = byte() % 11
            b = byte() % (11 - a)
            c = 10 - a - b
        else:
            a, b, c = byte() % 12, byte() % 12, byte() % 12
        w.append((0x40C8, bytes([a, b, c])))
    elif op == 5:
        cls = byte() % 18 if byte() < 0xE0 else byte()
        regs['eax'] = cls
        w.append((0x01A4 + 5, bytes([cls])))
        w.append((0x01A4 + 0x6E, bytes(byte() if byte() < 0x20 else 0 for _ in range(40 * 19))))
        w.append((0x01AA, bytes([byte() % 4])))
    elif op == 6:
        w.append((0x01A8, bytes([byte() % 9, byte() % 18 | (byte() & 0x20), byte() % 4, byte() & 1])))
        w.append((0x01CD, bytes(byte() for _ in range(16))))
        w.append((0x1299, bytes([byte() & 1])))
        w.append((0x05C8, pk(word())))
    elif op == 7:
        w.append((0x01A8, bytes([byte() % 9, byte() % 18 | (byte() & 0x20), byte() % 4, byte() & 1])))
        rolled = [byte() % 101 for _ in range(8)]
        now = [min(100, r + (byte() % 8 if byte() < 0x80 else 0)) if byte() < 0xF0 else byte() for r in rolled]
        w.append((0x01CD, bytes(now)))
        w.append((0x01D5, bytes(rolled)))
        w.append((0x129C, bytes([byte() % 26])))
        w.append((0x9686, pk(0x19 if byte() < 0xC0 else byte() % 40)))
        w.append((0x1296, bytes([byte() % 8])))
        w.append((0x1297, bytes([0x38 + 8 * (byte() % 8)])))
        w.append((0x05C8, pk(word())))
        for _ in range(1 + byte() % 6):
            if byte() < 0x50:
                clicks.append((0, 1 + byte() % 8))
            else:
                clicks.append((1, byte() if byte() < 0x20 else 0x30 + byte() % 0x48))
    elif op == 8:
        w.append((0x01A9, bytes([byte() % 18 | (byte() & 0x20)])))
        w.append((0x01CD, bytes(byte() for _ in range(16))))
        w.append((0x01F3, pk(word() % 30 if byte() < 0xE0 else word())))
        w.append((0x01FD, pk(word() % 60)))
        w.append((0x01FF, pk(word() % 60 if byte() < 0xE0 else word())))
        w.append((0x050A, bytes([byte(), byte(), byte()])))
    elif op == 9:
        w.append((0x129A, bytes([byte() % 10 if byte() < 0xF0 else byte()])))
        w.append((0x01A8, bytes([byte() % 9 if byte() < 0xF0 else byte()])))
        w.append((0x01AB, bytes([byte() & 1 if byte() < 0xE0 else byte()])))
        w.append((0x0FD4, bytes([byte()])))
    elif op == 10:
        for at in (0x05C8, 0xA902, 0x05B1, 0x12BA):
            w.append((at, pk(word())))
        w.append((0x05AD, pk4(word() | word() << 16)))
    elif op == 11:
        w.append((0x3B07, bytes([byte() & 1 if byte() < 0xE0 else byte()])))
        for at in (0xA902, 0x05C8, 0xCD00, 0x1042):
            w.append((at, pk(word())))
        w.append((0x05CC, bytes(byte() for _ in range(14))))
        w.append((0x1AB5, pk4(word() | word() << 16)))
        w.append((0x0A20, bytes([byte()])))
    elif op == 12:
        w.append((0x0A36, bytes(byte() for _ in range(16))))
        w.append((0x0A22, bytes(byte() for _ in range(20))))
        w.append((0x05E6, bytes([byte() % 5])))
        w.append((0x05E4, bytes([byte()])))
        w.append((0xBB0A, bytes([byte()])))
        w.append((0xC398, pk(word())))
    return w, regs, clicks


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    a.init_heap()
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    # DGROUP's relocated words back to the image's own (the port's).
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    RETF, RET = b'\xcb', b'\xc3'
    for seg, off, code in [
            # Screens, images, text, fonts, waits, music.
            (0, 0x541E, RETF), (0, 0x9466, RETF), (0, 0x9426, RETF), (0, 0x9AE6, RETF), (0, 0x9AF8, RETF),
            (0, 0x5454, RETF), (0, 0x5998, RETF), (0, 0x6BDE, RETF), (0, 0x5C86, RETF), (0, 0x9326, RETF),
            (0x2F9F, 0x0438, RETF), (0x1194, 0x5E21, RET), (0x1194, 0x5CD9, RET), (0x1194, 0x12BE, RET),
            (0x1194, 0x157E, RET), (0x1194, 0x1354, RET), (0x1194, 0x149D, RET), (0x1194, 0x0DE1, RET),
            (0x1194, 0x12E6, RET), (0x1194, 0x55B6, RET), (0x1194, 0x158F, RET)]:
        a.patch(seg, off, code)
    state = {'log': []}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])

    def stub(seg, off, far, fn):
        a.patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    stub(0x1194, 0x59CC, False, lambda: log(1, reg('si')))
    stub(0x1194, 0x0DC7, False, lambda: log(2))
    stub(0x1194, 0x1695, False, lambda: log(3))
    stub(0, 0x5D70, True, lambda: log(4))
    stub(0x2294, 0x11F2, True, lambda: log(5))
    stub(0x2294, 0x01F1, True, lambda: log(6))
    stub(0, 0x3C4C, True, lambda: log(7, reg('si')))
    stub(0x21EE, 0x0134, True, lambda: log(8))
    stub(0x29CF, 0x0691, True, lambda: log(9))
    question = open(os.path.join(GAME, next(n for n in os.listdir(GAME) if n.upper() == 'QUESTION.TXT')), 'rb').read()
    snap = a.snapshot()
    zero = bytes(0x10000)
    qtext = (question + bytes(0x10000))[:0x10000]
    rng = random.Random(0x1994_2498)
    total = int(os.environ.get('N', 3900))
    for n in range(total):
        op = n % OPS
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        mu.mem_write(a.lin(QSEG, 0), qtext)
        mu.mem_write(a.lin(FRAME, 0), zero)
        writes, regs, clicks = setup(seed, op)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        state['log'] = []
        before = a.seg_bytes(a.ds)
        base = dict(eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, eflags=2)
        try:
            ret = ''
            if op == 0:
                a.run_range(0x1194, 0x24A6, 0x2512, **base)
            elif op == 1:
                a.call_near(0x222C, 0x057F, **base)
            elif op == 2:
                r = a.call_near(0x222C, 0x0525, **dict(base, eax=regs['eax']))
                ret = f' {mu.reg_read(UC_X86_REG_DI):x}'
            elif op == 3:
                # The question found first, as the quiz does (0525).
                a.call_near(0x222C, 0x0525, **dict(base, eax=regs['eax']))
                di = mu.reg_read(UC_X86_REG_DI)
                before = a.seg_bytes(a.ds)
                a.run_range(0x222C, 0x02D4, 0x02DE, **dict(base, es=QSEG, edi=di))
            elif op == 4:
                r = a.call_near(0x222C, 0x005B, **base)
                ret = f' {r["ax"]:x}'
            elif op == 5:
                a.call_near(0x1194, 0x26DC, **dict(base, eax=regs['eax']))
            elif op == 6:
                a.call_near(0x1194, 0x2AC7, **base)
            elif op == 7:
                for kind, v in clicks:
                    if kind == 0:
                        a.w16(0xB7C6, v)
                        a.call_far(0x1194, 0x14C5, **base)
                    else:
                        a.w16(0x8F94, v)
                        a.call_far(0x1194, 0x138E, **base)
            elif op == 8:
                a.call_near(0x1194, 0x2D28, **base)
                a.call_near(0x1194, 0x2BAE, **base)
            elif op == 9:
                a.run_range(0x1194, 0x2C72, 0x2C95, **base)
            elif op == 10:
                a.run_range(0x1194, 0x257B, 0x25B7, **base)
            elif op == 11:
                a.call_far(0x21EE, 0x016C, **base)
            elif op == 12:
                a.run_range(0, 0x0C66, 0x0C95, **base)
            # The loader's last file name (A908, far) holds DGROUP as the
            # emulator relocated it: put back to the image's.
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, DGROUP)
            # Hooks the code wrote into segment 0000 (as the emulator
            # relocated it): the port's segment 0.
            for at in (0xCD34, 0xCD50, 0xCD54):
                if a.r16(at) == LOAD_SEG and before[at:at + 2] != struct.pack('<H', LOAD_SEG):
                    a.w16(at, 0)
            after = a.seg_bytes(a.ds)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before, after)
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
            digests = [fnv(changes(before, after)), fnv(changes(qtext, a.seg_bytes(QSEG))),
                       fnv(b''.join(struct.pack('<H', v & 0xFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests) + ret
        except (Fault, UcError):
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'newgame | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
