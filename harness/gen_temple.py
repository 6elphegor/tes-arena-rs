"""Emit test vectors for the temple (src/shops.rs `temple`, 1194:C258)
from the original code.

Usage: gen_temple.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/temple.rs) the
player's level, health, gold, afflictions and blessing, the time (now
and then a holiday's day from the list at 33AD), the last donation's
time, and the effect slots at 660 (some on the player); and the
front end's answers: the menu's picks (ending with "Exit"), the yes or
no answers, the donations typed and the list's picks.

What the port leaves to `ShopHooks` is a stub: a code hook at its first
instruction, patched to RET or RETF, logs the call and gives the
scripted answer:

  1 message (SI)          1194:4D31     2 menu (SI, BX, DI) AX  1194:020E
  3 dialog ([13D3], DI) AX 1194:4E88    4 list (SI, CX, DI) AX  1194:5734
  (the dialog's DI logged only for a number, kind 2)
  5 expand (DI)           2DA2:000A     6 quest check           1194:D961
  7 quests                1194:8F1E

A typed donation is written at DI by the dialog's stub. The screens,
images, fonts and the greeting (1194:5496, 54B3, 4B60, 4B8E, 4BFF,
5F8C, 5FC0; 0000:8FC4, 77EC, 9466, 541E, 9AE6, 9AF8) return at once.

  temple | seed | result

Results: "stuck", or the FNV-1a digests of the changed DGROUP bytes
and of the log. "Cure all" jumps into code that isn't (see
src/shops.rs); there the emulator ends with a fault or a hang depending
on its translation cache (the first such case in a process faults), so
which one a case gets depends on how the run was split. Both are written
"stuck", so the file is the same however it is sharded; the port gives
None for either.
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

TYPED = [b'', b'abc', b'0', b'7', b'25', b'60', b'150', b'1000', b'9999', b'-5', b'4x']


def setup(seed, holidays):
    """(writes, answers): writes as (offset, bytes, where in them DGROUP's
    segment goes or None); `holidays` the first 16 words at 33AD.
    Mirrored in tests/temple.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pk4 = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x01AA, bytes([byte() % 30 if byte() < 0xF0 else 0xFF]), None))
    mx = word() % 400
    w.append((0x01FF, pk(mx), None))
    w.append((0x01FD, pk(mx if byte() < 0x40 else word() % 400), None))
    w.append((0x05C2, pk4(word() % 3000 if byte() < 0xC0 else word() | word() << 16), None))
    w.append((0x05B1, pk(word() & 0xFFF8 | byte() & 7), None))
    w.append((0x05C6, pk(word()), None))
    if byte() < 0x60:
        day = holidays[byte() % 16]
    else:
        day = word() % 360
    t = (byte() % 8) * 0x7E900 + (day & 0xFFFF) * 0x5A0 + word() % 0x5A0
    w.append((0x05CC, pk4(t), None))
    k = byte() % 4
    if k == 0:
        last = 0
    elif k == 1:
        last = t - word() % 0x5A0
    elif k == 2:
        # More than a day ago, now and then exactly a day.
        last = t - 0x5A0 - (0 if byte() < 0x40 else 1 + word() % 0x500)
    else:
        last = word() | word() << 16
    w.append((0x13CD, pk4(last), None))
    w.append((0xA91A, pk(word()), None))
    w.append((0x0D39, bytes([byte()]), None))
    w.append((0x13C9, bytes([byte()]), None))
    for i in range(9):
        w.append((0x01A4 + 0x40F + i, bytes([byte() % 4]), None))
    for i in range(16):
        slot = bytearray(byte() for _ in range(0x3C))
        on_player = byte() < 0x80
        if on_player:
            slot[0x28:0x2A] = pk(0x01A4)
        slot[0x3A] = byte() % 12 if byte() < 0xE0 else 0xFF
        w.append((0x0660 + 0x3C * i, bytes(slot), 0x2A if on_player else None))
    menus = [(0, 1, 2, 3, 0xFFFF, 0, 1, 2)[byte() % 8] for _ in range(6)]
    yes = [byte() & 1 for _ in range(6)]
    typed = [TYPED[byte() % len(TYPED)] for _ in range(6)]
    lists = [0xFFFF if byte() < 0x30 else byte() % 8 for _ in range(6)]
    return w, (menus, yes, typed, lists)


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
    for seg, off, code in [(0x1194, 0x5496, RETF), (0x1194, 0x54B3, RETF), (0x1194, 0x4B60, RET),
                           (0x1194, 0x4B8E, RET), (0x1194, 0x4BFF, RET), (0x1194, 0x5F8C, RET),
                           (0x1194, 0x5FC0, RET), (0, 0x8FC4, RETF), (0, 0x77EC, RETF), (0, 0x9466, RETF),
                           (0, 0x541E, RETF), (0, 0x9AE6, RETF), (0, 0x9AF8, RETF)]:
        a.patch(seg, off, code)
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def stub(seg, off, far, fn):
        a.patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def take(k, default):
        q = state['answers'][k]
        i = state['n'][k]
        state['n'][k] += 1
        return q[i] if i < len(q) else default

    def menu():
        log(2, reg('si'), reg('bx'), reg('di'))
        wreg('ax', take(0, 3))

    def dialog():
        # DI is the buffer for a number; otherwise left from before.
        kind = mu.mem_read(a.lin(a.ds, 0x13D3), 1)[0]
        log(3, kind, reg('di') if kind == 2 else 0)
        if kind == 2:
            t = take(2, b'')
            mu.mem_write(a.lin(a.ds, reg('di')), t + b'\0')
            wreg('ax', 0)
        else:
            wreg('ax', take(1, 1))

    def lst():
        log(4, reg('si'), reg('cx'), reg('di'))
        v = take(3, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(reg('cx'), 1))

    def expand():
        di = reg('di')
        log(5, di)
        while mu.mem_read(a.lin(a.ds, di), 1)[0] != 0:
            di = (di + 1) & 0xFFFF
        wreg('di', di)

    stub(0x1194, 0x4D31, False, lambda: log(1, reg('si')))
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x4E88, False, dialog)
    stub(0x1194, 0x5734, False, lst)
    stub(0x2DA2, 0x000A, True, expand)
    stub(0x1194, 0xD961, False, lambda: log(6))
    stub(0x1194, 0x8F1E, False, lambda: log(7))
    cover = set()
    if os.environ.get('COVER'):
        for seg, lo, hi in ((0x1194, 0xC258, 0xC69F), (0x1194, 0x4A4A, 0x4AA7), (0x1194, 0x4E54, 0x4E88),
                            (0x2EBD, 0x0004, 0x0084), (0x2DA2, 0x111C, 0x1156), (0x1194, 0x485C, 0x487E), (0, 0x52BA, 0x52F2)):
            l0 = a.lin(LOAD_SEG + seg, 0)
            mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, d: cover.add(addr - d), l0, l0 + lo, l0 + hi)
    snap = a.snapshot()
    holidays = [a.r16(0x33AD + 2 * k) for k in range(16)]
    rng = random.Random(0x1994_C258)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        writes, answers = setup(seed, holidays)
        for off, data, ds_at in writes:
            if ds_at is not None:
                data = data[:ds_at] + struct.pack('<H', a.ds) + data[ds_at + 2:]
            mu.mem_write(a.lin(a.ds, off), data)
        state.update(log=[], answers=answers, n=[0, 0, 0, 0])
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x1194, 0xC258, eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, fs=0, gs=0, eflags=2)
            after = a.seg_bytes(a.ds)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before, after)
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
            digests = [fnv(changes(before, after))]
            digests.append(fnv(b''.join(struct.pack('<H', v & 0xFFFF) for v in state['log'])))
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, UcError):
            # UcError: a call through a table entry that isn't code.
            res = 'stuck'
        except AssertionError:
            res = 'stuck'
        out.write(f'temple | {seed:x} | {res}\n')
    out.close()
    if cover:
        with open(os.environ['COVER'], 'w') as f:
            for x in sorted(cover):
                f.write(f'{x:04x}\n')


if __name__ == '__main__':
    main()
