"""Emit test vectors for clicking on people and things (src/talk.rs
`actor_click`, 1194:DB6C, with the object click 292B:06DC) from the
original code.

Usage: gen_talk.py ACD_unpacked.EXE GAME_DIR OUT

The game's heap runs (the dialogue frees and allocates the window's
scratch buffer, 1194:5E49 and 5E54). Everything the port leaves to
`TalkHooks` is a stub: a code hook at its first instruction, patched to
RET or RETF, logs the call and its arguments and gives the scripted
answer of the case (AX, the carry, SI and BX). The answers come from the
case's stream after its set-up (`answers`, mirrored in tests/talk.rs):

  1 message box (SI)       1194:59CC        2 text ([137A], AX)  1194:A374
  3 menu (SI, BX, DI) AX   1194:020E        4 list (SI, CX) AX   1194:5734
  5 way (SI, AX, BX)       23F1:00A3        6 loot (AX)          1194:7B16
  7 sound (AX, CX)         23BA:002A        8 expand (DI)        2DA2:000A
  9 touch cast (SI) CF     29CF:0000       10 quest item (SI) CF 1194:9F04
 11 quest person           1194:D593       12 palace             1194:8F1E
 13 quest offer CF         1194:CD9A       14 quest chest (SI) CF 2294:0F33
 15 shop ([D48])           the 869F table  16 wild flat (SI)     2294:0DD0
 17 dungeon entrance       0000:4EFD       18 wild way (AX)      217B:0682
 19 square text (AX) CF SI BX  0000:06BE

The text substitution stub moves DI to the string's NUL, as the port's
default does. The fonts (0000:9AE6, 9AF8), the cursor's drawing
(0000:59B5), the mouse wait (1194:5E21) and 0000:08EC return at once.
0000:1242's `cdq` takes EAX's sign: EAX is made AX sign-extended there,
as the port assumes.

  talk | seed | result

Results: "fault" / "hang" / "lowmem", or the FNV-1a digests of the
changed DGROUP bytes (offset, value), the map and floor planes, the
scratch block, DATA:7387, and the log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn.x86_const import *
from emu import Fault, LOAD_SEG, STUB_SEG, DGROUP
from emu import changes
from emu import mine
from dos import DosArena
from gen_attack import fnv, stream

WSEG = 0x6000
FSEG = 0x7000
CSEG = 0x8000
PSEG = 0x5C00
DATA = 0x3276
SCRATCH = 0x400


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes, then the
    answers. Mirrored in tests/talk.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    W = lambda off, v: w.append((None, off, pk(v)))
    B = lambda off, v: w.append((None, off, pb(v)))
    near = lambda c: (c + (word() % 0x380) - 0x1C0) & 0xFFFF
    close = lambda c: (c + (word() % 0x200) - 0x100) & 0xFFFF
    px = 0x1000 + word() % 0x2000
    py = 0x1000 + word() % 0x2000
    W(0xC3D0, px)
    W(0xC3D4, py)
    W(0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1))
    base = 0x20 + byte() % 0x40
    W(0xC38E, base)
    for o in range(0xC934, 0xC98F):
        B(o, byte() % 0x40)
    B(0xC942, 0xFF if byte() < 0x20 else byte() % 0x40)
    a46 = byte() % 9
    B(0x0A46, a46)
    for i in range(10):
        B(0x8223 + i, (0, 5, 7, 1)[byte() % 4])
        if i < 8:  # (the city names follow at 80F4)
            B(0x80EC + i, a46 if byte() < 0x80 else byte() % 9)
    W(0xC398, (4 if byte() < 0x80 else 0) | (0x8000 if byte() & 1 else 0))
    W(0xC39A, (0x200 if byte() < 0x50 else 0) | 0x80 | (0x1000 if byte() & 1 else 0))
    W(0x05BE, 0x80 if byte() < 0x40 else 0)
    W(0x05C8, 0x4000 if byte() < 0x20 else 0)
    W(0x05CA, (1 if byte() < 0x50 else 0) | (2 if byte() & 1 else 0))
    B(0x05E6, (0, 3, 4, 4)[byte() % 4])
    B(0x0D48, (1, 4, 5, 6, 7, 2)[byte() % 6])
    B(0x0F76, byte() & 1)
    B(0x0F77, byte() % 10)
    if byte() < 0x80:
        w.append(('name', 0, b''))
    for o in (0x0F78, 0x0F79, 0x0F7B, 0x0F7D):
        B(o, byte() & 1)
    B(0x0A47, byte() % 4)
    B(0x0A4C, byte())
    B(0x01AA, byte() % 30)
    B(0x01AB, byte() & 1)
    W(0x0151, word())
    W(0x0DC4, (0, 0, 0x7D0, word())[byte() % 4])
    W(0xA902, byte() & 1)
    W(0x1030, word())
    W(0x91F6, word())
    W(0x91F8, word())
    for o in (0x01A4 + 0x2A, 0x01A4 + 0x2C, 0x01A4 + 6):
        B(o, byte())
    B(0x01A4 + 5, byte() % 18)
    for i in range(0x20):
        W(0x0F88 + 2 * i, 0xFFFF if byte() < 0xC0 else ((near(py) >> 7) << 8) + ((near(px) >> 7) << 1))
    slots = [0x0AB3 + 0x1C * i for i in range(23)]
    for k in range(8):
        at = 0x0622 + 6 * k
        W(at, slots[15 + byte() % 8] if byte() < 0x80 else 0xFFFF)
        W(at + 2, 0xC766 + 4 * (byte() % 0x20) + 0x389A)
        B(at + 4, 0x7B if byte() < 0x60 else byte())
    corpse = byte() % 0x40
    B(0xC936, corpse)
    for i, si in enumerate(slots):
        W(si, close(px))
        W(si + 4, close(py))
        k = byte() % 4
        if i >= 15:
            spr = (0xC0 + byte() % 6, (corpse + base) & 0xFF, 0xC5, byte())[k]
        else:
            spr = (0xC0 + byte() % 6, 0xC0 + byte() % 6, (base + byte() % 0x40) & 0xFF, byte())[k]
        B(si + 0xC, spr)
        W(si + 0xF, word() & (0xFFFF if byte() < 0x40 else 0xBFFF))
        W(si + 0x12, 0x500 * (i - 15) if i >= 15 else word())
    for k in range(8):
        rec = 0x500 * k
        for i in range(0x20):
            w.append((CSEG, rec + i, pb(byte())))
        w.append((CSEG, rec + 0x40D, pk(word())))
    rec = 0x3D96
    W(rec, close(px))
    W(rec + 4, close(py))
    kinds = (0xC93B, 0xC93C, 0xC93F, 0xC940, 0xC942, 0xC943, 0xC935, 0xC934)
    k = byte() % 13
    if k >= 11:
        B(rec + 0xC, base + byte() % 3)
    else:
        B(rec + 0xC, (byte() % 0x40 + base) & 0xFF if k >= 10 else 0)
    w.append(('flat', rec + 0xC, pb(k)))
    if k < 8:
        w.append(('kind', kinds[k], b''))
    elif k == 8:
        w.append(('kind', 0xC944 + byte() % 16, b''))
    elif k == 9:
        w.append(('kind', 0xC987 + byte() % 8, b''))
    W(rec + 0xF, word())
    W(rec + 0x12, word())
    W(0xCB5E, (0, slots[byte() % 15], slots[15 + byte() % 8], rec, rec, rec, 0xD7DE)[byte() % 7])
    for dy in range(-8, 9):
        for dx in range(-8, 9):
            sq = ((((py >> 7) + dy) << 8) + (((px >> 7) + dx) << 1)) & 0xFFFF
            v = byte()
            if v < 0x60:
                w.append((WSEG, sq, pk(0x8000 | word() & 0xFF)))
            elif v < 0x90:
                w.append((WSEG, sq, pk(word() & 0x7FFF)))
            if byte() < 0x40:
                w.append((FSEG, sq, pk(word())))
    for i, (ptr, cnt) in enumerate(((0x1AB5, 0x1387), (0x1004, 0x1389), (0x1008, 0x138B))):
        n = byte() % 6
        W(cnt, n)
        W(ptr, 0x100 * i)
        W(ptr + 2, PSEG)
        for e in range(n):
            at = 0x100 * i + 4 * e
            r = (41, 5)[byte() & 1]
            w.append((PSEG, at, bytes([((px >> 7) + byte() % r - r // 2) & 0xFF, ((py >> 7) + byte() % r - r // 2) & 0xFF,
                                       byte() % 8, byte() % 6])))
    B(0x138D, byte())
    B(0x138E, byte())
    W(0xC36B, byte())
    W(0xC36D, byte())
    B(0xC343, 1 + byte() % 4)
    gx = gy = 0
    for i in range(4):
        if not (i and byte() < 0x40):  # else the one before again
            gx, gy = near(px), near(py)
        W(0xC344 + 2 * i, gx)
        W(0xC34C + 2 * i, gy)
    for k in range(4):
        at = 0x0DCA + 0x29 * k
        W(at + 4, word())
        r = (257, 9)[byte() & 1]
        B(at + 0x10, ((px >> 7) + word() % r - r // 2) & 0xFF)
        B(at + 0x11, ((py >> 7) + word() % r - r // 2) & 0xFF)
        B(at + 0x1E, byte())
    for i in range(0x34):
        B(0x0F3E + i, byte())
    W(0x0F3E + 4, 0 if byte() < 0xC0 else word())
    menus = [(0, 1, 2, 3, 0xFFFF, 0, 1, 2)[byte() % 8] for _ in range(8)]
    lists = [0xFFFF if byte() < 0x20 else byte() % 11 for _ in range(8)]
    cfs = [byte() & 1 for _ in range(16)]
    texts = [(0xC766 + 4 * (byte() % 0x20), byte()) for _ in range(4)]
    return w, (menus, lists, cfs, texts)


class Script:
    def __init__(self, answers):
        self.menus, self.lists, self.cfs, self.texts = (list(x) for x in answers)
        self.n = [0, 0, 0, 0]
        self.log = []

    def take(self, k, q, default):
        i = self.n[k]
        self.n[k] += 1
        return q[i] if i < len(q) else default


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    a.init_heap()
    mu = a.mu
    RETF, RET = b'\xcb', b'\xc3'
    for seg, off, code in [(0, 0x9AE6, RETF), (0, 0x9AF8, RETF), (0, 0x59B5, RET), (0x1194, 0x5E21, RET),
                           (0, 0x08EC, RETF)]:
        a.patch(seg, off, code)
    script = [None]
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def carry(cf):
        f = reg('eflags')
        wreg('eflags', f | 1 if cf else f & ~1)

    def stub(seg, off, far, fn):
        a.patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(script[0]), None, lin, lin)

    def log(*v):
        script[0].log.extend(v)

    def cf_answer(sc):
        return sc.take(2, sc.cfs, 1)

    def menu(sc):
        log(3, reg('si'), reg('bx'), reg('di'))
        wreg('ax', sc.take(0, sc.menus, 3))

    def lst(sc):
        log(4, reg('si'), reg('cx'))
        wreg('ax', sc.take(1, sc.lists, 0xFFFF))

    def expand(sc):
        di = reg('di')
        log(8, di)
        es = reg('es')
        while mu.mem_read(a.lin(es, di), 1)[0] != 0:
            di = (di + 1) & 0xFFFF
        wreg('di', di)

    def flagged(k, *regs):
        def f(sc):
            log(k, *(reg(r) for r in regs))
            carry(cf_answer(sc))
        return f

    def square_text(sc):
        log(19, reg('ax'))
        cf = cf_answer(sc)
        carry(cf)
        if not cf:
            rec, bx = sc.take(3, sc.texts, (0xC766, 0))
            wreg('si', rec)
            wreg('bx', bx)

    def shop(sc):
        log(15, a.mu.mem_read(a.lin(a.ds, 0x0D48), 1)[0])

    stub(0x1194, 0x59CC, False, lambda sc: log(1, reg('si')))
    stub(0x1194, 0xA374, False, lambda sc: log(2, a.r16(0x137A), reg('ax')))
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x5734, False, lst)
    stub(0x23F1, 0x00A3, True, lambda sc: log(5, reg('si'), reg('ax'), reg('bx')))
    stub(0x1194, 0x7B16, True, lambda sc: log(6, reg('ax')))
    stub(0x23BA, 0x002A, True, lambda sc: log(7, reg('ax'), reg('cx')))
    stub(0x2DA2, 0x000A, True, expand)
    stub(0x29CF, 0x0000, True, flagged(9, 'si'))
    stub(0x1194, 0x9F04, True, flagged(10, 'si'))
    stub(0x1194, 0xD593, True, lambda sc: log(11))
    stub(0x1194, 0x8F1E, False, lambda sc: log(12))
    stub(0x1194, 0xCD9A, False, flagged(13))
    stub(0x2294, 0x0F33, True, flagged(14, 'si'))
    for off in (0xB93C, 0xC258, 0xA73E, 0xF48E):
        stub(0x1194, off, True, shop)
    stub(0x2294, 0x0DD0, True, lambda sc: log(16, reg('si')))
    stub(0, 0x4EFD, True, lambda sc: log(17))
    stub(0x217B, 0x0682, True, lambda sc: log(18, reg('ax')))
    stub(0, 0x06BE, True, square_text)
    # 0000:1242's cdq: EAX as AX sign-extended.
    lin = a.lin(LOAD_SEG, 0x122A)
    mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: wreg('eax', reg('ax') | (0xFFFF0000 if reg('ax') & 0x8000 else 0)),
                None, lin, lin)
    # The DGROUP words the loader relocates: E680 copies the list's
    # region (85D3.. runs into the far pointers at 869F) to the scratch
    # block, where they are put back image-relative.
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    relocs = set()
    for off_, seg_ in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        p = seg_ * 16 + off_ - DGROUP * 16
        if 0 <= p < 0x10000:
            relocs.add(p)
    copies = []
    lin = a.lin(LOAD_SEG + 0x1194, 0xE6A0)
    mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: copies.append((reg('si'), reg('es'), reg('di'))), None, lin, lin)
    low = []
    mu.hook_add(UC_HOOK_MEM_READ, lambda mu, acc, addr, size, v, _: low.append(addr), None, 0, 0x3FFF)
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    a.w16(0x1042, CSEG)
    a.w16(0x91F0, 0)
    a.w16(0xCB8E, SCRATCH)
    r = a.call_far(0, 0x51E2, ax=SCRATCH)
    a.w16(0xBA54, r)
    a.w16(0xBA56, mu.reg_read(UC_X86_REG_EAX) >> 16)
    cover = set()
    if os.environ.get('COVER'):
        for seg, lo, hi in ((0x1194, 0xDB6C, 0xE814), (0x292B, 0x06DC, 0x0942)):
            l0 = a.lin(LOAD_SEG + seg, 0)
            mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, d: cover.add(addr), None, l0 + lo, l0 + hi)
    snap = a.snapshot()
    rng = random.Random(0x1994_DB6C)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        if os.environ.get('SEED') and seed != int(os.environ['SEED'], 16):
            continue
        a.restore(snap)
        w, answers = setup(seed)
        flat_at = None
        for seg, off, data in w:
            if seg == 'flat':
                flat_at = off
            elif seg == 'name':
                # The quest's city name (80F4, by F77) made the current
                # one's (A22).
                at, n = 0x80F4, mu.mem_read(a.lin(a.ds, 0x0F77), 1)[0]
                for _ in range(n):
                    while mu.mem_read(a.lin(a.ds, at), 1)[0] != 0:
                        at += 1
                    at += 1
                d = 0x0A22
                while True:
                    c = mu.mem_read(a.lin(a.ds, at), 1)
                    mu.mem_write(a.lin(a.ds, d), bytes(c))
                    at, d = at + 1, d + 1
                    if c[0] == 0:
                        break
            elif seg == 'kind':
                b = (mu.mem_read(a.lin(a.ds, off), 1)[0] + a.r16(0xC38E)) & 0xFF
                mu.mem_write(a.lin(a.ds, flat_at), bytes([b]))
            else:
                mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        script[0] = Script(answers)
        copies.clear()
        before = a.seg_bytes(a.ds)
        low.clear()
        for r_ in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
            wreg(r_, 0)
        wreg('fs', a.ds)
        wreg('gs', a.ds)
        try:
            a.call_far(0x1194, 0xDB6C)
            res = None
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        if res is None and low:
            res = 'lowmem'
        if res is None:
            after = bytearray(a.seg_bytes(a.ds))
            for at in (0x061C,):
                v = struct.unpack_from('<H', after, at)[0]
                if v == a.ds:
                    struct.pack_into('<H', after, at, DGROUP)
                elif v == LOAD_SEG + DATA:
                    struct.pack_into('<H', after, at, DATA)
            # The guards' hook (0000:3F0C), its segment relocated.
            if struct.unpack_from('<H', after, 0x67EC)[0] == LOAD_SEG:
                struct.pack_into('<H', after, 0x67EC, 0)
            ch = changes(before, after)
            sseg, soff = a.r16(0xBA56), a.r16(0xBA54)
            scratch = bytearray(mu.mem_read(a.lin(sseg, soff), SCRATCH))
            if copies and copies[-1][1:] == (sseg, soff):
                src = copies[-1][0]
                for i in range(0, 0x384 - 1):
                    if (src + i) & 0xFFFF in relocs:
                        v = struct.unpack_from('<H', scratch, i)[0]
                        struct.pack_into('<H', scratch, i, (v - LOAD_SEG) & 0xFFFF)
            g = mu.mem_read(a.lin(LOAD_SEG + DATA, 0x7387), 1)[0]
            logb = b''.join(struct.pack('<H', v & 0xFFFF) for v in script[0].log)
            res = (f'{fnv(ch):x} {fnv(a.seg_bytes(WSEG)):x} {fnv(a.seg_bytes(FSEG)):x} '
                   f'{fnv(scratch):x} {g:x} {fnv(logb):x}')
        if os.environ.get('DUMP'):
            sys.stderr.write(f'log {[hex(v) for v in script[0].log]}\n')
            aft = a.seg_bytes(a.ds)
            for i in range(0x10000):
                if aft[i] != before[i]:
                    sys.stderr.write(f'{i:04x} {before[i]:02x} {aft[i]:02x}\n')
            sseg, soff = a.r16(0xBA56), a.r16(0xBA54)
            sys.stderr.write(f'scratch {sseg:x}:{soff:x} {bytes(mu.mem_read(a.lin(sseg, soff), SCRATCH)).hex()}\n')
        out.write(f'talk | {seed:x} | {res}\n')
    out.close()
    if cover:
        import capstone
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_16)
        for seg, lo, hi in ((0x1194, 0xDB6C, 0xE814), (0x292B, 0x06DC, 0x0942)):
            l0 = a.lin(LOAD_SEG + seg, 0)
            code = bytes(mu.mem_read(l0 + lo, hi - lo))
            for i in md.disasm(code, lo):
                if l0 + i.address not in cover:
                    sys.stderr.write(f'{seg:04X}:{i.address:04X} {i.mnemonic} {i.op_str}\n')


if __name__ == '__main__':
    main()
