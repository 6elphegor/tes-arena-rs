"""Emit test vectors for the mages' guild (src/guild.rs `mages_guild`,
1194:A73E) from the original code.

Usage: gen_guild.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/guild.rs) the state
the guild reads: a fight or the guards coming (FFB, 680F), the player
(gold 5C2, class 1A9, level 1AA, personality 1D3, the lockpicking bytes,
the items at 212, the spells known at 50A), the date (5CC, a quarter on
the guilds' holidays), the shop's level and factors (13CC, 13CA, 13CB),
the city's byte the level rolls read (0A22 + 25h), the place's seed and
the seller's (A4C, A46, A56), the player's x and height (the stock's
file's name), the spells' records (3276: 55h bytes each, a name at +34h
and a cost at +32h, for the first 30h); and the answers the front end
gives: the menus' picks, the lists' picks, yes or no, the counts typed,
the stock's file (none, or its bytes: a time, a count, records), the
haggles (the price agreed, or walked away) and the spell screen's
clicks. EMS is on (dos.py `enable_ems`, the frame at FRAME).

Everything the port leaves to `GuildHooks` is a stub: a code hook at
its first instruction, patched to RET or RETF, logs the call and gives
the scripted answer (the carry clear unless scripted):

  1 quest check (D961)    2 quests (8F1E)
  3 file exists (0:600C: CF; then reads, 0:6BEC, from the file's
    bytes, AX the bytes read, not logged: the port reads them itself)
  5 write (0:6DDC: CX, then the bytes' digest's low dword)
  8 menu (020E: SI BX DI) AX          9 list (5734: DS SI CX) AX,
                                        DS and ES DGROUP after
  10 message box (59CC: SI)           11 message (4D31: SI)
  12 dialog (4E88: [13D3]; 1: AX, 2: the text at DS:DI)
  13 picture (4B8E: SI)   14 haggle (8020: EAX BX CX SI DH) EAX, CF
  16 spell screen (68A3: the record at DS:SI copied to 57E6 here)
  17 click (0:9466: AFF9, 8F92, 8F94 written)    18 spellmaker (6064)

The screen's routines (0000:541E, 9AE6, 9AF8, 5454, A611, A6B5, 67C4,
677A, 6742; 25FD:0844; 1194:5496, 54B3, 5E49, 5E54, 4B60, 4BFF, 5F8C,
545C, 5FC0, 5E63, 5E77) return at once; 0000:8FC4 and 77EC too, with
DGROUP in DS as they leave it. DGROUP's relocated
words are put back to image-relative values, as in the port.

  guild | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (the guards' hook segment 67EC, the file opener's
DS at A90A and a name's AH at 1258 put back image-relative; 12CE, the
spells' saved SP, as it was), of DATA's
changes below DGROUP (it starts at DATA:92A0), of the frame and EMS bank 18h, and
of the log.
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

FRAME = 0xA000
DATA = 0x3276

COUNTS = (b'1', b'3', b'10', b'0', b'', b'2x', b'255', b'300', b'7', b'12')
# The spell screen's clicks: (buttons, x, y): back, buy, exit, misses.
CLICKS = ((1, 0x30, 0xC0), (1, 0x90, 0xC0), (1, 0x130, 0xC0), (0, 0x90, 0xC0),
          (1, 0x90, 0xC0), (1, 0x90, 0xC0), (1, 0x90, 0xC0), (1, 0x90, 0xC0),
          (1, 0x90, 0x10), (1, 0x70, 0xC0), (1, 0x100, 0xC0), (1, 0x2, 0xC0),
          (1, 0x69, 0xB8), (1, 0x78, 0xB8), (1, 0xBE, 0xB8), (1, 0x11D, 0xB8), (1, 0x136, 0xB8), (1, 0x137, 0xC0))


def setup(seed, holidays):
    """(writes, data writes, answers). Mirrored in tests/guild.rs, draw
    for draw (no tuple draws two)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x0FFB, pk(0 if byte() < 0xF0 else word())))
    w.append((0x680F, bytes([0 if byte() < 0xF0 else byte()])))
    w.append((0xA91A, pk(word())))
    w.append((0x91F6, pd(dword())))
    w.append((0x91F0, pk(0)))
    k = byte()
    if k < 0x40:
        gold = byte() % 60
    elif k < 0xC0:
        gold = word() % 0x1000
    else:
        gold = dword() % 0x100000
    w.append((0x05C2, pd(gold)))
    w.append((0x01A9, bytes([(byte() & 0x20) | (byte() % 0x12)])))
    w.append((0x01AA, bytes([byte() % 0x20])))
    w.append((0x01D3, bytes([byte()])))
    for at in (0x01A4 + 0x2A, 0x01A4 + 0x2C):
        w.append((at, bytes([byte() % 0x40])))
    if byte() < 0x40 and holidays:
        day = holidays[byte() % len(holidays)]
        now = day * 1440 + word() % 1440 + (byte() % 4) * 518400
    else:
        now = dword() % 0x800000
    w.append((0x05CC, pd(now)))
    w.append((0x13CC, bytes([byte() % 0x18])))
    w.append((0x13CA, bytes([byte()])))
    w.append((0x13CB, bytes([byte()])))
    w.append((0x0A22 + 0x25, bytes([byte() % 3])))
    w.append((0x0A4C, pd(dword())))
    w.append((0x0A46, bytes([byte() % 8])))
    w.append((0x0A56, pd(dword())))
    w.append((0xC3D0, pk(word())))
    w.append((0xC3D2, pk(word() % 0x200)))
    w.append((0x0D48, bytes([7])))
    w.append((0x13F4, bytes([0 if byte() < 0xC0 else byte()])))
    w.append((0x13FA, pd(dword() % 0x20000)))
    # The player's items: packed from 212 (40 of 13h bytes).
    n_items = (0, 1, 5, 12, 25, 40)[byte() % 6]
    items = bytearray(0x28 * 0x13)
    for i in range(n_items):
        it = bytearray(0x13)
        kind = byte() % 4
        if kind == 0:
            # A potion: category 3, a type of 15, a count.
            flags = 0x40 | 0x0C | 1 | (2 if byte() < 0x80 else 0)
            it[0] = byte() % 0x0F
            struct.pack_into('<H', it, 1, byte() % 4)
        elif kind == 1:
            flags = 0x40 | 0x04 | (2 if byte() < 0x40 else 0)
            it[0] = byte() % 0x12
        elif kind == 2:
            flags = 0x40 | (2 if byte() < 0x40 else 0)
            it[0] = byte() % 0x0B
            it[0x10] = byte() % 3
        else:
            # An accessory: a ring or bracelet (kinds 0..2).
            flags = 0x40 | 0x08 | 1 | (byte() % 3) << 4 | (2 if byte() < 0x80 else 0)
            it[0] = byte() % 4
            it[0x10] = byte() % 3
            it[0x0F] = byte() % 3
        it[0x0E] = flags
        struct.pack_into('<I', it, 0x0A, (byte(), word(), word() % 3000)[byte() % 3])
        items[i * 0x13:(i + 1) * 0x13] = it
    w.append((0x0212, bytes(items)))
    known = byte() % 8
    w.append((0x050A, bytes([known] + [byte() % 0x2F for _ in range(known)])))
    # The spells' records in 3276.
    dw = []
    for n in range(0x30):
        if byte() < 0xC0:
            name = bytes(0x61 + byte() % 26 for _ in range(1 + byte() % 8))
        else:
            name = b''
        dw.append((n * 0x55 + 0x34, name + b'\0'))
        dw.append((n * 0x55 + 0x32, pk(word() % 0x800 if byte() < 0xC0 else word())))
    menus = []
    for _ in range(10):
        k = byte() % 8
        menus.append(0xFFFF if k == 7 else k % 5)
    lists = []
    for _ in range(6):
        lists.append(0xFFFF if byte() < 0x30 else byte() % 16)
    yes = [byte() & 1 for _ in range(3)]
    counts = [COUNTS[byte() % len(COUNTS)] for _ in range(4)]
    haggles = []
    for _ in range(4):
        cf = byte() < 0x40
        k = byte()
        if k < 0x30:
            eax = gold
        elif k < 0xC0:
            eax = word() % 0x800
        else:
            eax = dword() % 0x100000
        haggles.append((cf, eax))
    clicks = [CLICKS[byte() % len(CLICKS)] for _ in range(6)]
    # The stock's file.
    k = byte()
    if k < 0x60:
        file = None
    else:
        age = (byte() % 0x5A0, 0x5A0 + word(), 0x5A0 - 1, 0x5A0, word() | 0x80000000)[byte() % 5]
        f = bytearray(struct.pack('<I', (now - age) & 0xFFFFFFFF))
        count = (0, 1, 3, 8, byte())[byte() % 5]
        f.append(count)
        for i in range(count):
            r = bytearray(0x0B)
            r[0] = byte() % 3 if byte() < 0xF8 else 3
            r[1] = byte() % 6
            r[2] = byte() % 3
            r[3] = byte() % 8
            r[4] = byte() % 0x20
            struct.pack_into('<I', r, 5, dword() % 0x10000)
            r[9] = byte()
            r[10] = byte()
            f += r
        cut = byte()
        if cut < 0x20:
            f = f[:byte() % (len(f) + 1)]
        file = bytes(f)
    # Aimed at edges: the place's seed + 1000 at 0 (made 1), the price a
    # potion's health takes at 50000, an item's value about 10 (a tenth
    # of it, or 1).
    if byte() < 0x20:
        w.append((0x0A4C, pd(0xFFFFFC18)))
    if byte() < 0x30:
        w.append((0x13F4, bytes([1])))
        w.append((0x13FA, pd(0xC350 + byte() % 3 - 1)))
    if byte() < 0x40 and n_items:
        i = byte() % n_items
        w.append((0x0212 + i * 0x13 + 0x0A, pd(9 + byte() % 3)))
        w.append((0x0212 + i * 0x13 + 0x0E, bytes([items[i * 0x13 + 0x0E] | 2])))
    return w, dw, (menus, lists, yes, counts, haggles, clicks, file)


def guild_holidays(a):
    """The days of the year that are the guilds' holidays (the shops'
    byte 7 in the table at 3353, the days at 33AD, in order)."""
    days = []
    for i in range(0x40):
        d = a.r16(0x33AD + 2 * i)
        if d > 0x158:
            break
        if bytes(a.mu.mem_read(a.lin(a.ds, 0x3353 + 6 * i), 1))[0] == 7:
            days.append(d)
    return days


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    a.GAME_INT3 = set(a.GAME_INT3) | {(0x1194, 0xB7EA)}
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    a.enable_ems(FRAME)
    RETF, RET = b'\xcb', b'\xc3'
    for off in (0x541E, 0x9AE6, 0x9AF8, 0x5454, 0xA611, 0xA6B5, 0x67C4, 0x677A, 0x6742):
        a.patch(0, off, RETF)
    a.patch(0x25FD, 0x0844, RETF)
    for off in (0x5496, 0x54B3, 0x5E49, 0x5E54):
        a.patch(0x1194, off, RETF)
    for off in (0x4B60, 0x4BFF, 0x5F8C, 0x545C, 0x5FC0, 0x5E63, 0x5E77):
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

    def exists():
        log(3)
        carry(state['file'] is None)

    def read():
        cx = reg('cx')
        f = state['file'] or b''
        p = state['pos']
        got = f[p:p + cx]
        state['pos'] = p + len(got)
        mu.mem_write(a.lin(reg('ds'), reg('si')), got) if got else None
        wreg('ax', len(got))

    def write():
        cx = reg('cx')
        data = bytes(mu.mem_read(a.lin(reg('ds'), reg('si')), cx)) if cx else b''
        log(5, cx, fnv(data) & 0xFFFFFFFF)

    def menu():
        log(8, reg('si'), reg('bx'), reg('di'))
        keys = bytes(mu.mem_read(a.lin(a.ds, reg('bx')), 8)).split(b'\0')[0]
        v = take(0, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(len(keys), 1))

    def lst():
        ds = reg('ds')
        log(9, 1 if ds == a.ds else 2, reg('si'), reg('cx'))
        v = take(1, 0xFFFF)
        cx = reg('cx')
        wreg('ax', v if v == 0xFFFF else v % max(cx, 1))
        # 1194:5734 returns with DS and ES DGROUP.
        wreg('ds', a.ds)
        wreg('es', a.ds)

    def dialog():
        kind = bytes(mu.mem_read(a.lin(a.ds, 0x13D3), 1))[0]
        log(12, kind, reg('di') if kind == 2 else 0)
        if kind == 2:
            t = take(3, b'0')
            mu.mem_write(a.lin(a.ds, reg('di')), t + b'\0')
            wreg('ax', 0)
        else:
            wreg('ax', take(2, 1))

    def haggle():
        eax = reg('eax')
        log(14, eax & 0xFFFF, eax >> 16, reg('bx'), reg('cx'), reg('si'))
        cf, v = take(4, (True, 0))
        wreg('eax', v)
        carry(cf)

    def spell_screen():
        log(16)
        rec = bytes(mu.mem_read(a.lin(reg('ds'), reg('si')), 0x55))
        mu.mem_write(a.lin(a.ds, 0x57E6), rec)

    def click():
        # Only the spell screen's poll (its return at 1194:B273); 9466 is
        # the screen's elsewhere.
        sp = reg('sp')
        if (a.r16(sp, seg=reg('ss')), a.r16(sp + 2, seg=reg('ss'))) != (0xB273, LOAD_SEG + 0x1194):
            return
        bt, x, y = take(5, (1, 0x130, 0xC0))
        log(17, bt, x, y)
        mu.mem_write(a.lin(a.ds, 0xAFF9), bytes([bt]))
        a.w16(0x8F92, x)
        a.w16(0x8F94, y)

    # 0000:8FC4 and 77EC load DGROUP into DS and leave it there.
    stub(0, 0x8FC4, True, lambda: wreg('ds', a.ds))
    stub(0, 0x77EC, True, lambda: wreg('ds', a.ds))
    stub(0x1194, 0xD961, False, lambda: log(1))
    stub(0x1194, 0x8F1E, False, lambda: log(2))
    stub(0, 0x600C, True, exists)
    stub(0, 0x6BEC, True, read)
    stub(0, 0x6DDC, True, write)
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x5734, False, lst)
    stub(0x1194, 0x59CC, False, lambda: log(10, reg('si')))
    stub(0x1194, 0x4D31, False, lambda: log(11, reg('si')))
    stub(0x1194, 0x4E88, False, dialog)
    stub(0x1194, 0x4B8E, False, lambda: log(13, reg('si')))
    stub(0x1194, 0x8020, False, haggle)
    stub(0x1194, 0x68A3, True, spell_screen)
    stub(0, 0x9466, True, click)
    stub(0x1194, 0x6064, True, lambda: log(18))

    # 1194:5ED1 with the pick (13C8) not below the count: the entries
    # after it number 65536 less, and moving them runs dword copies
    # across the frame's end, a #GP on the CPU (unicorn reads on into the
    # next 64 KB): stuck.
    def take_out(mu, addr, size, _):
        if bytes(mu.mem_read(a.lin(a.ds, 0x13C8), 1))[0] >= reg('cx'):
            a.fault = 13
            mu.emu_stop()
    lin = a.lin(LOAD_SEG + 0x1194, 0x5ED1)
    mu.hook_add(UC_HOOK_CODE, take_out, None, lin, lin)
    if os.environ.get('TRAIL'):
        from unicorn import UC_HOOK_BLOCK
        trail = state.setdefault('trail', [])
        seen = state.setdefault('seen', {})

        def blk(mu, addr, size, _):
            trail.append(addr)
            if len(trail) > 60:
                trail.pop(0)
            if addr not in seen:
                seen[addr] = len(seen)
        mu.hook_add(UC_HOOK_BLOCK, blk)
    if os.environ.get('PROBE'):
        for spec in os.environ['PROBE'].split(','):
            sg, of = (int(x, 16) for x in spec.split(':'))
            lin = a.lin(LOAD_SEG + sg, of)

            def probe(mu, addr, size, _, spec=spec):
                print(f'PROBE {spec} ds {reg("ds"):04x} es {reg("es"):04x} ax {reg("ax"):04x} '
                      f'eax {reg("eax"):08x} 13c9 {bytes(mu.mem_read(a.lin(a.ds, 0x13C9), 1))[0]:02x}', file=sys.stderr)
            mu.hook_add(UC_HOOK_CODE, probe, None, lin, lin)
    if os.environ.get('WATCH'):
        from unicorn import UC_HOOK_MEM_WRITE
        for w in os.environ['WATCH'].split(','):
            lin = a.lin(a.ds, int(w, 16))

            def watch(mu, acc, addr, size, value, _, w=w):
                print(f'WATCH {w} <- {value:x} at {reg("cs") - LOAD_SEG:04x}:{reg("ip"):04x}', file=sys.stderr)
            mu.hook_add(UC_HOOK_MEM_WRITE, watch, None, lin, lin)
    holidays = guild_holidays(a)
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA, as the game does
    snap = a.snapshot()
    rng = random.Random(0x1994_A73E)
    total = int(os.environ.get('N', 4000))
    only = os.environ.get('SEED')
    for n in range(total):
        seed = rng.getrandbits(32)
        if only:
            if n:
                break
            seed = int(only, 16)
        if not mine(n, total):
            continue
        a.restore(snap)
        writes, dwrites, answers = setup(seed, holidays)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        for off, data in dwrites:
            mu.mem_write(a.lin(LOAD_SEG + DATA, off), data)
        menus, lists, yes, counts, haggles, clicks, file = answers
        state.get('trail', []).clear()
        state.get('seen', {}).clear()
        state.update(log=[], answers=(menus, lists, yes, counts, haggles, clicks), file=file, pos=0, n=[0] * 6)
        # Each case from the same registers and flags.
        for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
            wreg(r, 0)
        wreg('eflags', 0x0002)
        before = (a.seg_bytes(a.ds), a.seg_bytes(LOAD_SEG + DATA), a.seg_bytes(FRAME), a.ems_bank(0x18))
        try:
            a.call_far(0x1194, 0xA73E)
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', ds, 0x67EC)[0] == LOAD_SEG and before[0][0x67EC:0x67EE] != ds[0x67EC:0x67EE]:
                ds[0x67EC:0x67EE] = struct.pack('<H', 0)
            # The file opener keeps the caller's DS (0000:6B26), DGROUP's
            # relocated here.
            if struct.unpack_from('<H', ds, 0xA90A)[0] == a.ds and before[0][0xA90A:0xA90C] != ds[0xA90A:0xA90C]:
                ds[0xA90A:0xA90C] = struct.pack('<H', DGROUP)
            # 12CE holds SP (1194:B1ED), the caller's stack: not compared.
            ds[0x12CE:0x12D0] = before[0][0x12CE:0x12D0]
            # A bracelet's name keeps AH at 1258: DGROUP's segment's high
            # byte (1194:B7D8 sets only AL).
            if ds[0x1258] == a.ds >> 8 and before[0][0x1258] != ds[0x1258]:
                ds[0x1258] = DGROUP >> 8
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before[0], bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[0][o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
                c = changes(before[3], a.ems_bank(0x18))
                for i in range(0, min(len(c), 30000), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'BANK {o:04x} {before[3][o]:02x} {v:02x}', file=sys.stderr)
                c = changes(before[1], a.seg_bytes(LOAD_SEG + DATA))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'DATA {o:04x} {before[1][o]:02x} {v:02x}', file=sys.stderr)
            if os.environ.get('STATS'):
                lg = state['log']
                tags = set()
                for k, v in ((14, 'haggle'), (16, 'spell'), (18, 'spellmaker')):
                    if k in lg:
                        tags.add(v)
                if before[0][0x0212:0x0212 + 0x28 * 0x13] != bytes(ds[0x0212:0x0212 + 0x28 * 0x13]):
                    tags.add('inventory')
                if before[0][0x05C2:0x05C6] != bytes(ds[0x05C2:0x05C6]):
                    tags.add('gold')
                if state['file'] is None:
                    tags.add('made')
                for t, v in ((0x198C, 'no-gold'), (0x19DE, 'full'), (0x714F, 'identified'), (0x20FA, 'steal-failed'),
                             (0x7210, 'free'), (0x6F5C, 'known')):
                    if t in lg:
                        tags.add(v)
                if before[0][0x680F] != ds[0x680F]:
                    tags.add('guards')
                if before[0][0x050A] != ds[0x050A]:
                    tags.add('spell-bought')
                print('STATS', ' '.join(sorted(tags)), file=sys.stderr)
            # DATA below DGROUP only (from 92A0 on it is DGROUP).
            digests = [fnv(changes(before[0], bytes(ds))), fnv(changes(before[1][:0x92A0], a.seg_bytes(LOAD_SEG + DATA)[:0x92A0])),
                       fnv(changes(before[2], a.seg_bytes(FRAME))), fnv(a.ems_bank(0x18)),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                cs, ip = reg('cs') - LOAD_SEG, reg('ip')
                print(f'{seed:x}: {e!r} at {cs:04x}:{ip:04x} log {state["log"]}', file=sys.stderr)
                print('table', [hex(a.r16(0x7143 + 2 * i)) for i in range(3)], 'ax', hex(reg('ax')), 'bx', hex(reg('bx')), 'ds', hex(reg('ds')), file=sys.stderr)
                if 'trail' in state:
                    print(' '.join(f'{x - LOAD_SEG * 16:05x}' for x in state['trail'][-40:]), file=sys.stderr)
                if 'seen' in state:
                    order = sorted(state['seen'], key=state['seen'].get)
                    print('first', ' '.join(f'{x - LOAD_SEG * 16:05x}' for x in order[-120:]), file=sys.stderr)
        out.write(f'guild | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
