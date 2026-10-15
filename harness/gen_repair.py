"""Emit test vectors for the equipment store's repairs (src/store.rs
`store_repair`, 1194:C6AC) from the original code.

Usage: gen_repair.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/repair.rs) the state
the repairs read: the RNG's seed (91F6), the player (gold 5C2, strength
1CD, class), the player's 40 item slots (212), the date (5CC), the five
repair records (76E3: taken or not, ready before, at or after now, by
less than 6 hours, about a day, or more; their items, now and then a
quest piece), the word at 1042 (the formatting scans back past 1044 into
it) and A902's bit 10h; and the answers the front end gives: the
dialogs' (AX, and the number typed for kind 2) and the lists' picks.

Everything the port leaves to `StoreHooks` is a stub: a code hook at its
first instruction, patched to RET or RETF, logs the call and gives the
scripted answer:

  5 list (5734: SI CX DI) AX, DS = ES = DGROUP as the list leaves them
  6 message (4D31: SI)           7 message box (59CC: SI)
  12 picture (4B8E: SI)
  13 text (A374: 137A, 137E, 101C, AX, DI): the text's first variant from
     TEMPLATE.DAT (read here, as the test reads it) written at DI
  14 dialog (4E88: 13D3, and DI for kind 2) AX; kind 2 writes the
     number typed at DI

The screen's routines (0000:8FC4, 77EC, 9466, 541E, 9AE6, 9AF8; 1194:
5496, 54B3, 5E21, 4B60, 5F8C, 545C, 5FC0) return at once. The word at
7865 (where 1194:CCFD saves SP) is put back before the digest.

  repair | seed | result

Debugging: ONLY=seed runs that case alone; DUMP=seed prints the case's
changed DGROUP bytes and log.

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes and of the log.
"""
import os
import random
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import *
from emu import Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream
from gen_store import StoreDos

FSEG = 0x5C00
WSEG = 0x7C00
REPAIRS = 0x76E3


def templates(game):
    """The repairs' texts, 589h..594h: each one's first variant (up to
    its first '&'), from TEMPLATE.DAT."""
    d = open(os.path.join(game, 'TEMPLATE.DAT'), 'rb').read()
    out = {}
    for m in re.finditer(rb'#(\d+)\r?\n([^#]*)', d):
        n = int(m.group(1))
        if 0x589 <= n <= 0x594:
            out[n] = m.group(2).split(b'&')[0]
    return out


def item(byte, word, dword, special):
    """A 13h-byte item (as gen_store's player items, a quest piece when
    `special`)."""
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    rec = bytearray(0x13)
    cat = byte() % 4
    if special:
        ty = 0x80 | byte() % 0x10
    elif cat == 1:
        ty = byte() % 18
    elif cat == 0:
        ty = byte() % 11
    elif cat == 3:
        ty = byte() % 15
    else:
        ty = byte() % 8
    kind = byte() % 3
    flags = 0x40 | cat << 2 | kind << 4
    if byte() < 0x60:
        flags |= 1
    if byte() < 0x30:
        flags |= 2
    if byte() < 0x40:
        flags |= 0x80
    rec[0] = ty
    rec[1:3] = pk((byte() % 6) << 8 if byte() < 0xC0 else word())
    rec[3] = byte()
    rec[4] = byte()
    rec[5] = byte()
    hi = word() % 0x2000 + 1
    k = byte()
    # A quarter whole (the condition at its maximum), a few with +3 equal
    # to +8's low byte.
    health = hi if k < 0x40 else 0 if k < 0x50 else word() % 0x2000
    rec[6:8] = pk(health)
    rec[8:10] = pk(hi)
    if byte() < 0x20:
        rec[3] = hi & 0xFF
    kk = byte()
    rec[0x0A:0x0E] = pd(byte() % 10 if kk < 0x30 else word() % 0x4000 if kk < 0xE0 else dword())
    rec[0x0E] = flags
    rec[0x0F] = 0xFF if byte() < 0x80 else byte() % 0x18
    rec[0x10] = 0xFF if byte() < 0x80 else byte() % 8
    rec[0x11] = byte()
    rec[0x12] = byte()
    return bytes(rec)


def setup(seed):
    """(writes, answers). Mirrored in tests/repair.rs, draw for draw (no
    tuple draws two)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x91F6, pd(dword())))
    w.append((0x91F0, pk(0)))
    k = byte()
    if k < 0x40:
        gold = byte() % 0x40
    elif k < 0xC0:
        gold = word() % 0x1000
    else:
        gold = byte() | byte() << 8 | byte() << 16
    w.append((0x05C2, pd(gold)))
    w.append((0x01CD, bytes([0xFF if byte() < 0x80 else byte()])))
    w.append((0x01A4 + 0x05, bytes([byte() % 0x12])))
    w.append((0x01A9, bytes([byte()])))
    # An eighth with the inventory full.
    n = 40 if byte() < 0x20 else byte() % 41
    for i in range(40):
        rec = item(byte, word, dword, byte() < 4) if i < n else bytes(0x13)
        w.append((0x0212 + 0x13 * i, rec))
    day = byte() % 0x168
    now = (byte() % 4) * 518400 + day * 1440 + word() % 1440 + 0x10000
    w.append((0x05CC, pd(now)))
    taken = byte() % 6
    for i in range(5):
        at = REPAIRS + 0x18 * i
        rec = bytearray(0x18)
        if i < taken or byte() < 0x10:
            rec[0] = 1 if byte() < 0xE0 else byte()
            k = byte() % 10
            if k == 0:
                delta = -(word() % 0x3000) - 1
            elif k == 8:
                delta = word() % 0x168
            elif k == 9:
                delta = word()
            else:
                delta = (0, 0, 1, 0x168, 0x169, 0x59F, 0x5A0, 0x5A1)[k]
            rec[1:5] = pd(now + delta)
            rec[5:] = item(byte, word, dword, byte() < 0x30)
        w.append((at, bytes(rec)))
    hi = byte()
    w.append((0x1042, pk((0x26 if hi == 0x25 else hi) << 8 | byte() % 0x10)))
    w.append((0xA902, pk(0x10 if byte() & 1 else 0)))
    dialogs = []
    for _ in range(10):
        k = byte()
        ax = (0, 1, 2, 0, 1)[k % 5] if k < 0xF0 else word()
        kk = byte()
        if kk < 0x20:
            typed = b''
        elif kk < 0x30:
            typed = b'0'
        elif kk < 0x40:
            typed = bytes([0x30 + byte() % 10, 0x41 + byte() % 26])
        else:
            typed = str(byte() % 10 if kk < 0xA0 else word() % 10000).encode()
        dialogs.append((ax, typed))
    lists = []
    for _ in range(6):
        lists.append(0xFFFF if byte() < 0x30 else byte())
    return w, (dialogs, lists)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    texts = templates(GAME)
    a = StoreDos(EXE, GAME)
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    RETF, RET = b'\xcb', b'\xc3'
    for off in (0x8FC4, 0x77EC, 0x9466, 0x541E, 0x9AE6, 0x9AF8):
        a.patch(0, off, RETF)
    for off in (0x5496, 0x54B3):
        a.patch(0x1194, off, RETF)
    for off in (0x5E21, 0x4B60, 0x5F8C, 0x545C, 0x5FC0):
        a.patch(0x1194, off, RET)
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def carry(cf):
        f = reg('eflags')
        wreg('eflags', f | 1 if cf else f & ~1)

    def stub(seg, off, far, fn):
        a.patch(seg, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + seg, off)

        def run(mu, addr, size, _):
            carry(False)
            fn()
        mu.hook_add(UC_HOOK_CODE, run, None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def take(k, default):
        q = state['answers'][k]
        i = state['n'][k]
        state['n'][k] += 1
        return q[i] if i < len(q) else default

    def lst():
        log(5, reg('si'), reg('cx'), reg('di'))
        v = take(1, 0xFFFF)
        n = reg('cx')
        wreg('ax', v if v == 0xFFFF else v % max(n, 1))
        wreg('ds', a.ds)
        wreg('es', a.ds)

    def text():
        number = a.r16(0x137A)
        log(13, number, mu.mem_read(a.lin(a.ds, 0x137E), 1)[0], mu.mem_read(a.lin(a.ds, 0x101C), 1)[0],
            reg('ax'), reg('di'))
        mu.mem_write(a.lin(a.ds, reg('di')), texts.get(number, b'') + b'\0')

    def dialog():
        kind = mu.mem_read(a.lin(a.ds, 0x13D3), 1)[0]
        log(14, kind)
        ax, typed = take(0, (2, b''))
        if kind == 2:
            log(reg('di'))
            mu.mem_write(a.lin(a.ds, reg('di')), typed[:4] + b'\0')
        wreg('ax', ax)

    stub(0x1194, 0x5734, False, lst)
    stub(0x1194, 0x4D31, False, lambda: log(6, reg('si')))
    stub(0x1194, 0x59CC, False, lambda: log(7, reg('si')))
    stub(0x1194, 0x4B8E, False, lambda: log(12, reg('si')))
    stub(0x1194, 0xA374, False, text)
    stub(0x1194, 0x4E88, False, dialog)
    snap = a.snapshot()
    rng = random.Random(0x1994_C6AC)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        if os.environ.get('ONLY') and os.environ['ONLY'] != f'{seed:x}':
            continue
        a.restore(snap)
        a.mh = {}
        a.deleted = []
        a.handles[5] = [b'', 0]
        a.w16(0xD806, WSEG)
        a.w16(0x9030, FSEG)
        writes, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        a.files = {}
        state.update(log=[], answers=answers, n=[0, 0])
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0x1194, 0xC6AC)
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            ds[0x7865:0x7867] = before[0x7865:0x7867]
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before, bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
            digests = [fnv(changes(before, bytes(ds))),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                print(f'{seed:x}: {e!r} at {reg("cs") - LOAD_SEG:04x}:{reg("ip"):04x} log {state["log"][:40]}', file=sys.stderr)
        out.write(f'repair | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
