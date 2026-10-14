"""Emit test vectors for the tavern (src/tavern.rs `tavern`, 1194:B93C)
from the original code.

Usage: gen_tavern.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/tavern.rs) the
state the tavern reads: a fight or the guards coming (FFB, 680F), the
player (gold 5C2, +4, +2Eh, the lockpicking bytes, attributes), the
drinks taken (1378, 1379), the lock tried (13CC), the date (5CC), the
city's record bytes the shop's level reads (0A22 + 25h), the places'
list the drinks' seed comes from (FD2, 1387, the far pointer at 1AB5 to
CSEG), the place (5E6, D48, 191D, C398, A47), the map (MAP1 at D806,
WSEG); and the answers the front end gives: the menus' picks, the
lists' picks, yes or no, the days typed, the room's file (found, its
hours and end) and the haggle (the price agreed, or walked away).

Everything the port leaves to `TavernHooks` is a stub: a code hook at
its first instruction, patched to RET or RETF, logs the call and gives
the scripted answer (the carry clear unless scripted):

  1 files (C0B1)          2 greeting (C12A)      3 quest (CED7)
  4 quests (8F1E)         5 tavern's quest (90B9)
  6 room read (C04C: CF; 12BC, 1019, 12CA)       7 room write (BA91)
  8 menu (020E: SI BX DI) AX                     9 list (5734: SI CX) AX
  10 message box (59CC: SI)                      11 message (4D31: SI)
  12 dialog (4E88: [13D3]; 1: AX, 2: the text at DS:DI)
  13 picture (4B8E: SI)   14 haggle (C14E: [1391]) EAX, CF
  15 rumours (E3F7)

The screen's routines (0000:8FC4, 77EC, 9466, 541E, 9AE6, 9AF8; 1194:
5496, 54B3, 5E21, 4B60, 5F8C, 545C, 5FC0) return at once. DGROUP's
relocated words are put back to image-relative values, as in the port.

  tavern | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (the guards' hook segment 67EC and the effects'
target segments put back image-relative), of the map's changes and of
the log.
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

CSEG = 0x6C00
WSEG = 0x7C00

DAYS = (b'1', b'3', b'10', b'0', b'', b'2x', b'255', b'300', b'7', b'12')


def setup(seed):
    """(writes, cseg bytes, answers). Mirrored in tests/tavern.rs, draw
    for draw (no tuple draws two)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x0FFB, pk(0 if byte() < 0xF0 else word())))
    w.append((0x680F, bytes([0 if byte() < 0xF0 else byte()])))
    w.append((0xA91A, pk(word())))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    w.append((0x91F0, pk(0)))
    k = byte()
    if k < 0x40:
        # A purse about a drink's price, for the price-equals-gold edge.
        gold = byte() % 40
    elif k < 0xC0:
        gold = word() % 0x800
    else:
        gold = byte() | byte() << 8 | byte() << 16
    w.append((0x05C2, pd(gold)))
    w.append((0x01A8, bytes([byte() & 1])))
    w.append((0x01D2, bytes([byte() % 0x80])))
    for at in (0x01A4 + 0x2A, 0x01A4 + 0x2C):
        w.append((at, bytes([byte() % 0x40])))
    w.append((0x01A4 + 0x05, bytes([byte() % 0x12])))
    if byte() < 0x80:
        w.append((0x01A4 + 0x06, bytes([byte() % 3])))
    else:
        w.append((0x01A4 + 0x06, bytes([byte() % 0x20])))
    w.append((0x01A4 + 0x2E, bytes([byte()])))
    for i in range(8):
        w.append((0x01A4 + 0x0E + i, bytes([byte()])))
        w.append((0x01A4 + 0x16 + i, bytes([byte()])))
    w.append((0x1378, bytes([byte() % 0x40])))
    w.append((0x1379, bytes([byte() % 6])))
    w.append((0x13CC, bytes([byte() % 0x18])))
    if byte() < 0x40:
        # The taverns' holidays: free drinks (days 0, 236), free rooms (45).
        day = (0, 45, 236)[byte() % 3]
        w.append((0x05CC, pd(day * 1440 + word() % 1440 + (byte() % 4) * 518400)))
    else:
        w.append((0x05CC, pd(byte() | byte() << 8 | (byte() % 0x10) << 16)))
    w.append((0x0A22 + 0x25, bytes([byte() % 3])))
    w.append((0x0D48, bytes([4])))
    w.append((0x05E6, bytes([4 if byte() < 0xC0 else 3])))
    w.append((0x191D, bytes([byte() & 1])))
    v = word() & ~4
    if byte() < 0x40:
        v |= 4
    w.append((0xC398, pk(v)))
    w.append((0x0A47, bytes([byte() % 4])))
    key = byte()
    w.append((0x0FD2, pk(key | byte() << 8)))
    count = byte() % 6
    w.append((0x1387, pk(count)))
    w.append((0x1AB5, pk(0x10)))
    w.append((0x1AB7, pk(CSEG)))
    cseg = bytearray(0x200)
    for i in range(0x80):
        cseg[i] = byte()
    for i in range(count):
        at = 0x10 + 4 * i
        if byte() < 0x60:
            cseg[at] = key >> 1
        else:
            cseg[at] = byte()
        cseg[at + 1] = byte()
        cseg[at + 2] = byte()
        cseg[at + 3] = byte()
    w.append((0xC943, bytes([byte()])))
    menus = []
    for _ in range(8):
        k = byte() % 8
        if k == 7:
            menus.append(0xFFFF)
        else:
            menus.append(k % 5)
    lists = []
    for _ in range(4):
        if byte() < 0x30:
            lists.append(0xFFFF)
        else:
            lists.append(byte() % 10)
    yes = [byte() & 1 for _ in range(3)]
    days = [DAYS[byte() % len(DAYS)] for _ in range(4)]
    room = (byte() < 0x60, word() % 0x80, byte() | byte() << 8 | (byte() % 0x10) << 16)
    haggles = []
    for _ in range(3):
        cf = byte() < 0x40
        k = byte()
        if k < 0x30:
            # The price agreed exactly the player's gold.
            eax = gold
        elif k < 0xC0:
            eax = word() % 0x400
        else:
            eax = byte() | byte() << 8 | byte() << 16
        haggles.append((cf, eax))
    return w, bytes(cseg), (menus, lists, yes, days, room, haggles)


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

    def stub(off, far, fn):
        a.patch(0x1194, off, RETF if far else RET)
        lin = a.lin(LOAD_SEG + 0x1194, off)

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

    def room_read():
        log(6)
        found, hours, end = state['room']
        if found:
            mu.mem_write(a.lin(a.ds, 0x12BC), b'\x01')
            a.w16(0x1019, hours)
            mu.mem_write(a.lin(a.ds, 0x12CA), struct.pack('<I', end))
        else:
            mu.mem_write(a.lin(a.ds, 0x12BC), b'\x00')
            carry(True)

    def menu():
        log(8, reg('si'), reg('bx'), reg('di'))
        # A pick among the menu's entries (one per key at BX).
        keys = bytes(mu.mem_read(a.lin(a.ds, reg('bx')), 8)).split(b'\0')[0]
        v = take(0, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(len(keys), 1))

    def lst():
        log(9, reg('si'), reg('cx'))
        wreg('ax', take(1, 0xFFFF))

    def dialog():
        kind = bytes(mu.mem_read(a.lin(a.ds, 0x13D3), 1))[0]
        # DI matters (and is logged) only for typing: the yes or no
        # dialog's is what earlier code left.
        log(12, kind, reg('di') if kind == 2 else 0)
        if kind == 2:
            t = take(3, b'0')
            mu.mem_write(a.lin(a.ds, reg('di')), t + b'\0')
            wreg('ax', 0)
        else:
            wreg('ax', take(2, 1))

    def haggle():
        log(14, a.r16(0x1391), a.r16(0x1393))
        cf, eax = take(4, (True, 0))
        wreg('eax', eax)
        carry(cf)

    stub(0xC0B1, False, lambda: log(1))
    stub(0xC12A, False, lambda: log(2))
    stub(0xCED7, False, lambda: log(3))
    stub(0x8F1E, False, lambda: log(4))
    stub(0x90B9, False, lambda: log(5))
    stub(0xC04C, False, room_read)
    stub(0xBA91, False, lambda: log(7, a.r16(0x1019), a.r16(0x12CA), a.r16(0x12CC)))
    stub(0x020E, True, menu)
    stub(0x5734, False, lst)
    stub(0x59CC, False, lambda: log(10, reg('si')))
    stub(0x4D31, False, lambda: log(11, reg('si')))
    stub(0x4E88, False, dialog)
    stub(0x4B8E, False, lambda: log(13, reg('si')))
    stub(0xC14E, False, haggle)
    stub(0xE3F7, False, lambda: log(15))
    if os.environ.get('TRAIL'):
        from unicorn import UC_HOOK_BLOCK
        trail = state.setdefault('trail', [])
        mu.hook_add(UC_HOOK_BLOCK, lambda mu, addr, size, _: (trail.append(addr), len(trail) > 60 and trail.pop(0)))
    snap = a.snapshot()
    rng = random.Random(0x1994_B93C)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        a.w16(0xD806, WSEG)
        writes, cseg, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(CSEG, 0), cseg)
        menus, lists, yes, days, room, haggles = answers
        # The answers by kind: menu 0, list 1, yes 2, days 3, haggle 4.
        state.get('trail', []).clear()
        state.update(log=[], answers=(menus, lists, yes, days, haggles), room=room, n=[0, 0, 0, 0, 0])
        before = (a.seg_bytes(a.ds), a.seg_bytes(WSEG))
        try:
            a.call_far(0x1194, 0xB93C)
            ds = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', ds, 0x67EC)[0] == LOAD_SEG and before[0][0x67EC:0x67EE] != ds[0x67EC:0x67EE]:
                ds[0x67EC:0x67EE] = struct.pack('<H', 0)
            # The effects' slots (16 of 3Ch at 0660): their target's
            # segment (+2Ah), DGROUP's relocated in the emulator.
            for i in range(16):
                at = 0x0660 + 0x3C * i + 0x2A
                if struct.unpack_from('<H', ds, at)[0] == a.ds and before[0][at:at + 2] != ds[at:at + 2]:
                    ds[at:at + 2] = struct.pack('<H', DGROUP)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before[0], bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[0][o]:02x} {v:02x}', file=sys.stderr)
            if os.environ.get('STATS'):
                lg = state['log']
                tags = set()
                for k, v in ((14, 'haggle'), (13, 'sneak'), (15, 'rumours')):
                    if k in lg:
                        tags.add(v)
                if 0x7361 in lg:
                    tags.add('sneak-failed')
                if 0x73C6 in lg or 0x110C in lg:
                    tags.add('sneak-room')
                if 0x198C in lg:
                    tags.add('no-gold')
                if 0x74FB in lg:
                    tags.add('free-room')
                if before[0][0x0660:0x0662] != bytes(ds[0x0660:0x0662]):
                    tags.add('drunk')
                if before[0][0x680F] != ds[0x680F]:
                    tags.add('guards')
                if 4164 in lg:
                    tags.add('drink')
                print('STATS', ' '.join(sorted(tags)), file=sys.stderr)
            digests = [fnv(changes(before[0], bytes(ds))), fnv(changes(before[1], a.seg_bytes(WSEG))),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                cs, ip = reg('cs') - LOAD_SEG, reg('ip')
                print(f'{seed:x}: {e!r} at {cs:04x}:{ip:04x} log {state["log"][:40]}', file=sys.stderr)
                if 'trail' in state:
                    print(' '.join(f'{x - LOAD_SEG * 16:05x}' for x in state['trail'][-40:]), file=sys.stderr)
        out.write(f'tavern | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
