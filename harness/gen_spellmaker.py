"""Emit test vectors for the spellmaker (src/spellmaker.rs `spellmaker`,
1194:6064 with SI the player, 1A4) from the original code.

Usage: gen_spellmaker.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/spellmaker.rs) the
state the spellmaker reads: the player's class (1A9: a quarter each the
three classes with factors, 24h, E5h, 22h, and any byte), level (1AA),
gold (5C2), spells known (50A), the save slot (1A6C), the screen's flags
(1001, 101D), the word %a shows (5873), A902 and 5C8, and the player's
spells (DATA 2A80: 32 slots, occupied at random, now and then all); and
the answers the front end gives: the clicks (each read on the main
screen as a region, 0..6, or a miss, 7, at an offset in it, its bottom
edge included, which misses; and on a form as a control's arrow, 2, 8, 5
or 6 rows below its top, up to 5 being up, its done box, or a miss), the lists'
and menus' picks and the names typed. Once the clicks run out the main
screen's "exit" or the form's done box is clicked.

EMS is off; the frame (9030) is at 6000h and the screen (A8FE) at
7000h:0; the heap runs from B000h (1194:55D9 clears A000h, the VGA's
memory in the game). GLOBAL.BSA is open (the forms' pictures).

The stubs (a code hook at the first instruction, patched to RET or
RETF; the log in the order of the calls):

  1 click (1194:5651: AX, BX the click; 5C8 bit 2 cleared): 1, form or 0,
    x, y
  2 list (1194:5734: SI CX) AX, DS and ES DGROUP after
  3 menu (1194:020E: SI BX DI) AX
  4 message box (1194:59CC: SI)
  5 name (0000:9EE5: the name written at DS:SI): 5, SI
  6 the spells' file (0000:9DD2: SI, CX, the bytes' digest's low dword)

The screen's routines return at once: 0000:541E, 5454, 6BDE, 9AE6, 9AF8,
5828, 5364, 8F26, 9E06, 9370, 5998, 599C, 9D84, 9466, 56C8, 5E3E, 9426;
25FD:0844.

  spellmaker | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (A876, the stack pointer 1194:6DD2 keeps, a
stand-in FFD0h once 6E1F has used it, as in the port; the file opener's DS at A90A and the description's pushed segments
put back image-relative: their high bytes where 6D87 stored DS),
of DATA's changes below DGROUP, and of the log.
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

DATA = 0x3276
FRAME = 0x6000
SCREEN = 0x7000
HEAP = (0xB000, 0xF000)
NAMES = (b'Fireball', b'', b'x', b'Healing Touch', b'A' * 30, b'Zap')
# The regions' boxes (DGROUP 5711): x0, y0, x1, y1 (y below y1).
REGIONS = ((0x45, 0x31, 0xC8, 0x39), (0x12, 0x48, 0x34, 0x4F), (0x12, 0x3C, 0x31, 0x45),
           (0xCB, 0x31, 0xF2, 0x38), (0x0A, 0xBC, 0x35, 0xC5), (0xCA, 0xBC, 0xF3, 0xC5),
           (0x11F, 0xBC, 0x131, 0xC5))


def setup(seed):
    """(writes, data writes, answers). Mirrored in tests/spellmaker.rs,
    draw for draw."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    k = byte() % 4
    cls = (0x24, 0xE5, 0x22, byte())[k] if k < 3 else byte()
    w.append((0x01A9, bytes([cls])))
    w.append((0x01AA, bytes([byte() % 0x20])))
    k = byte()
    if k < 0x30:
        gold = byte() % 40
    elif k < 0xC0:
        gold = word() % 0x2000
    else:
        gold = dword() % 0x100000
    w.append((0x05C2, pd(gold)))
    known = byte() % 8
    w.append((0x050A, bytes([known] + [byte() for _ in range(known)])))
    w.append((0x1A6C, bytes([byte() % 10])))
    w.append((0x1001, bytes([0xFF if byte() < 0x20 else byte() % 0xFF])))
    w.append((0x101D, bytes([0xFF if byte() < 0x20 else byte() % 0xFF])))
    w.append((0x5873, pk(word())))
    w.append((0xA902, pk(word())))
    w.append((0x05C8, pk(word())))
    dw = []
    full = byte() < 0x20
    for n in range(0x20):
        occupied = full or byte() < 0x60
        dw.append((0x2A80 + n * 0x55 + 0x34, bytes([0x61 + byte() % 26 if occupied else 0])))
    clicks = []
    for _ in range(byte() % 40):
        r = byte() % 16
        r = (1, 1, 1, 5, 5, 2, 3, 0, 4, 1, 5, 2, 6, 7, 1, 3)[r]
        clicks.append((r, byte(), byte(), byte() % 4, byte(), byte() % 4))
    lists = [0xFFFF if byte() < 0x18 else byte() for _ in range(24)]
    menus = [0xFFFF if byte() < 0x18 else byte() % 4 for _ in range(12)]
    names = [NAMES[byte() % len(NAMES)] for _ in range(4)]
    return w, dw, (clicks, lists, menus, names)


def click_at(a, entry, form):
    """The click an answer gives: on the main screen (form None) a region
    or a miss; on a form a control's arrow, its done box or a miss.
    Mirrored in tests/spellmaker.rs."""
    r, ox, oy, kind, idx, up = entry
    if form is None:
        if r == 7:
            return (0x10, 0x10)
        x0, y0, x1, y1 = REGIONS[r]
        return (x0 + ox % (x1 - x0 + 1), y0 + oy % (y1 - y0 + 1))
    rd = lambda o: a.r16(form + o, seg=LOAD_SEG + DATA)
    fx, fy = a.r16(0xA880), a.r16(0xA882)
    if kind < 2:
        n = rd(4)
        c = form + 0x12 + 0x12 * (idx % max(n, 1))
        return ((fx + rd(c - form) + 1) & 0xFFFF, (fy + rd(c - form + 2) + (2, 8, 5, 6)[up]) & 0xFFFF)
    if kind == 2:
        return ((fx + rd(0xA)) & 0xFFFF, (fy + rd(0xC)) & 0xFFFF)
    return ((fx + 0xF0) & 0xFFFF, (fy + 0x60) & 0xFFFF)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    # DGROUP's relocated words put back to image-relative values.
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    a.init_heap(*HEAP)
    a.w16(0x9030, FRAME)
    a.w16(0xA8FE, 0)
    a.w16(0xA900, SCREEN)
    RETF, RET = b'\xcb', b'\xc3'
    for off in (0x541E, 0x5454, 0x6BDE, 0x9AE6, 0x9AF8, 0x5828, 0x5364, 0x8F26, 0x9E06, 0x9370, 0x5998,
                0x599C, 0x9D84, 0x9466, 0x56C8, 0x5E3E, 0x9426):
        a.patch(0, off, RETF)
    a.patch(0x25FD, 0x0844, RETF)
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

    def click():
        sp = reg('sp')
        ret = a.r16(sp, seg=reg('ss'))
        form = a.r16(0xA888) if ret == 0x624C else None
        e = take(0, None)
        if e is None:
            if form is None:
                x, y = 0x128, 0xC0
            else:
                rd = lambda o: a.r16(form + o, seg=LOAD_SEG + DATA)
                x, y = (a.r16(0xA880) + rd(0xA)) & 0xFFFF, (a.r16(0xA882) + rd(0xC)) & 0xFFFF
        else:
            x, y = click_at(a, e, form)
        state['clicks'] += 1
        if state['clicks'] > 400:
            a.fault = 99
            mu.emu_stop()
            return
        log(1, form if form is not None else 0, x, y)
        a.w16(0x05C8, a.r16(0x05C8) & 0xFFFD)
        wreg('ax', x)
        wreg('bx', y)

    def lst():
        log(2, reg('si'), reg('cx'))
        v = take(1, 0xFFFF)
        cx = reg('cx')
        wreg('ax', v if v == 0xFFFF else v % max(cx, 1))
        wreg('ds', a.ds)
        wreg('es', a.ds)

    def menu():
        log(3, reg('si'), reg('bx'), reg('di'))
        keys = bytes(mu.mem_read(a.lin(a.ds, reg('bx')), 8)).split(b'\0')[0]
        v = take(2, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(len(keys), 1))

    def name():
        log(5, reg('si'))
        t = take(3, b'')
        mu.mem_write(a.lin(reg('ds'), reg('si')), t + b'\0')

    def spells():
        cx = reg('cx')
        data = bytes(mu.mem_read(a.lin(reg('es'), reg('di')), cx))
        log(6, reg('si'), cx, fnv(data) & 0xFFFFFFFF)

    # Where 1194:6D87 stores DS (MOV AX, DS then STOSW): the words'
    # high bytes, put back image-relative after unless written over
    # since (a later argument or a pick there).
    def ds_store(mu, addr, size, _):
        state['pending'] = (reg('di') + 1) & 0xFFFF

    for off in (0x6E96, 0x6F01, 0x6F2F, 0x6F57):
        lin = a.lin(LOAD_SEG + 0x1194, off)
        mu.hook_add(UC_HOOK_CODE, ds_store, None, lin, lin)

    def written(mu, access, addr, size, value, _):
        for k in range(size):
            o = addr + k - a.lin(a.ds, 0)
            if o == state.get('pending'):
                state['segs'].add(o)
                state['pending'] = None
            else:
                state['segs'].discard(o)
    from unicorn import UC_HOOK_MEM_WRITE
    mu.hook_add(UC_HOOK_MEM_WRITE, written, None, a.lin(a.ds, 0x1200), a.lin(a.ds, 0x1300))
    # A876 keeps SP across the description's sprintf (1194:6DD2, 6E1F)
    # and holds it after: a stand-in (FFD0h) from 6E23 on, as the port
    # leaves there, since the stack's place is the harness's.
    lin = a.lin(LOAD_SEG + 0x1194, 0x6E23)
    mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: a.w16(0xA876, 0xFFD0), None, lin, lin)
    stub(0x1194, 0x5651, False, click)
    stub(0x1194, 0x5734, False, lst)
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x59CC, False, lambda: log(4, reg('si')))
    stub(0, 0x9EE5, True, name)
    stub(0, 0x9DD2, True, spells)
    if os.environ.get('TRAIL'):
        from unicorn import UC_HOOK_BLOCK
        trail = state.setdefault('trail', [])

        def blk(mu, addr, size, _):
            trail.append(addr)
            if len(trail) > 60:
                trail.pop(0)
        mu.hook_add(UC_HOOK_BLOCK, blk)
    cover = set()
    if os.environ.get('COVER'):
        # The blocks run in 1194:6064..72A0, for a coverage report.
        from unicorn import UC_HOOK_BLOCK
        lo, hi = a.lin(LOAD_SEG + 0x1194, 0x6064), a.lin(LOAD_SEG + 0x1194, 0x72A0)
        mu.hook_add(UC_HOOK_BLOCK, lambda mu, addr, size, _: cover.add(addr - lo + 0x6064), None, lo, hi)
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA, as the game does
    snap = a.snapshot()
    rng = random.Random(0x1994_6064)
    total = int(os.environ.get('N', 3000))
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
        writes, dwrites, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        for off, data in dwrites:
            mu.mem_write(a.lin(LOAD_SEG + DATA, off), data)
        state.get('trail', []).clear()
        state.update(log=[], answers=answers, n=[0] * 4, clicks=0, segs=set(), pending=None)
        # Each case from the same registers and flags.
        for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
            wreg(r, 0)
        wreg('eflags', 0x0002)
        before = (a.seg_bytes(a.ds), a.seg_bytes(LOAD_SEG + DATA))
        try:
            a.call_far(0x1194, 0x6064, si=0x01A4)
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            # The file opener keeps the caller's DS (0000:6B26).
            if struct.unpack_from('<H', ds, 0xA90A)[0] == a.ds and before[0][0xA90A:0xA90C] != ds[0xA90A:0xA90C]:
                ds[0xA90A:0xA90C] = struct.pack('<H', DGROUP)
            # The description's arguments: DS stored as far pointers'
            # segments (its high byte, where nothing wrote over it since;
            # the picks at 1238 do write over their low bytes).
            for o in state['segs']:
                if ds[o] == a.ds >> 8:
                    ds[o] = DGROUP >> 8
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before[0], bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[0][o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
                c = changes(before[1][:0x92A0], a.seg_bytes(LOAD_SEG + DATA)[:0x92A0])
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'DATA {o:04x} {before[1][o]:02x} {v:02x}', file=sys.stderr)
            digests = [fnv(changes(before[0], bytes(ds))),
                       fnv(changes(before[1][:0x92A0], a.seg_bytes(LOAD_SEG + DATA)[:0x92A0])),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                cs, ip = reg('cs') - LOAD_SEG, reg('ip')
                print(f'{seed:x}: {e!r} at {cs:04x}:{ip:04x} log {state["log"][-20:]}', file=sys.stderr)
                if 'trail' in state:
                    print(' '.join(f'{x - LOAD_SEG * 16:05x}' for x in state['trail'][-40:]), file=sys.stderr)
        out.write(f'spellmaker | {seed:x} | {res}\n')
    out.close()
    if cover:
        open(os.environ['COVER'], 'w').write(' '.join(f'{x:04x}' for x in sorted(cover)))


if __name__ == '__main__':
    main()
