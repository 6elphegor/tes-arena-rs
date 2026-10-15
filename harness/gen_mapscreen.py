"""Emit test vectors for the travel map's screen (src/mapscreen.rs) from
the original code.

Usage: gen_mapscreen.py ACD_unpacked.EXE OUT

Ops, each a sixth of the cases:
  0 the map button (2542:03DC, `map_button` then `map_button_done`),
    the screen (26F7:0000) answering a scripted carry
  1 the screen's start (26F7:0000..0048 and 008C..009B, `travel_map_begin`)
  2 the screen's end (26F7:00CE..013D, `travel_map_end`), AX/BX/CX the
    location kept at its start; the result has the carry
  3 the journey shown (26F7:0483, `travel_journey`)
  4 a click on the map (26F7:0625, `travel_click`)
  5 the search (26F7:03A4, `travel_search`)

A case fills from a seed (`setup`, mirrored in tests/mapscreen.rs) the
flags (C39A, C398, 5C8, FFB), the current location (A46, A47, A4C,
A48/A4A), the province shown (47E1), the destination (1929.. 1946, 12A8),
the choice (6C28, 6C41, A8AE, A8BC, A8C4), the journey's state (1927,
3FF9, 3FF2, 12A2, 5E6), the clock (5CC), the mouse (8F92, 8F94, AFF9),
the province's 32 places (1445: names and positions) and the answers the
stubs give.

The stubs (patched to RET or RETF; the log in the order of the calls):
  1 a message box (1194:59C8: SI, the text's FNV-1a low dword)
  2 the screen (26F7:0000, from the button): the case's carry
  3 the palette's fade (0000:5E3E)
  4 the quest dungeon's picture (292B:0362: AX)
  5 the days (1194:2E69): 1927, 3FF9, 3FF2 and AX the case's
  6 the screen's bank shown (1194:4D74: BX)
  7 a province loaded (1194:2E14: AL)
  8 a bank mapped (0000:5C86: BX)
  9 the pick (26F7:03C4: AX in; AX the case's)
  10 the journey (26F7:0483)
  11 the name typed (1194:5C30: the case's name at 9C62)
  12 the list (1194:5730: SI, CX; AX the case's)
and silent: the view's and screens' routines, fonts and loads (0000:9A98,
8FC4, 9AE6, 756C, 9AF8, 7428, A611, A602, A5CD, A6B5, 9426, 9466, 7530,
9B4E, 66F2 (AX 30h), 9E06, 9D84, 5828, 710E; 1194:5E1D).

  mapscreen | seed | op | result

Results: "stuck", or the FNV-1a digests of the changed DGROUP bytes and
of the log, and for op 2 the carry.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import *
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream

FRAME = 0x6000
SCREEN = 0x7000
MARKS = 0x8000


def setup(seed):
    """(op, writes, answers). Mirrored in tests/mapscreen.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    letters = lambda n: bytes(0x41 + byte() % 26 if i == 0 else 0x61 + byte() % 26 for i in range(n))
    op = byte() % 6
    w = []
    w.append((0xC39A, pk(word())))
    w.append((0xC398, pk(word())))
    w.append((0x05C8, pk(word())))
    f = byte()
    w.append((0x0FFB, pk(0 if f < 0xA0 else 1 if f < 0xC0 else word())))
    w.append((0x0FCE, pk(word())))
    w.append((0xC994, bytes([byte()])))
    w.append((0x0A46, bytes([byte() % 9])))
    w.append((0x0A47, bytes([3 if byte() < 0x60 else byte() % 3])))
    w.append((0x0A4C, bytes([(0x20 + byte() % 4) if byte() < 0x60 else byte()])))
    w.append((0x0A48, pk(word() % 320)))
    w.append((0x0A4A, pk(word() % 200)))
    w.append((0x47E1, bytes([byte() % 9])))
    w.append((0x1929, letters(1 + byte() % 15) + b'\0'))
    w.append((0x1429, letters(1 + byte() % 12) + b'\0'))
    w.append((0x193D, bytes([byte() % 9])))
    w.append((0x1943, bytes([byte() % 4])))
    w.append((0x12A8, pk(8 if byte() < 0x40 else byte() % 9)))
    w.append((0x6C28, bytes([byte() & 1])))
    w.append((0x6C41, bytes([0 if byte() < 0xC0 else 1])))
    w.append((0xA8AE, pk(byte() % 0x30)))
    w.append((0xA8BC, pk(byte() % 0x30)))
    w.append((0xA8C4, bytes([byte()])))
    w.append((0x1927, pk(byte() % 60)))
    w.append((0x3FF9, pk(word())))
    w.append((0x3FF2, struct.pack('<I', word() << 4)))
    w.append((0x12A2, pk(byte() & 1)))
    w.append((0x05E6, bytes([2 if byte() < 0x60 else byte() % 4])))
    w.append((0x05CC, struct.pack('<I', word() << 8 | byte())))
    w.append((0x8F92, pk(word() % 320)))
    w.append((0x8F94, pk(word() % 200)))
    w.append((0xAFF9, bytes([0 if byte() < 0x80 else 1])))
    w.append((0x1032, pk(byte())))
    w.append((0x6636, bytes([byte()])))
    # The typed name's buffer, not empty past where the search clears it.
    w.append((0x9C62, bytes((i * 37 + 11) & 0xFF | 1 for i in range(0x200))))
    for i in range(32):
        rec = 0x1445 + i * 0x19
        name = letters(1 + byte() % 10)
        w.append((rec, name + b'\0'))
        w.append((rec + 0x14, pk(word() % 320) + pk(word() % 200)))
        w.append((rec + 0x18, bytes([byte()])))
    carry = byte() & 1
    days = (byte() % 50, word(), word() << 4)
    pick = 0xFFFF if byte() < 0x40 else byte() % 0x30
    typed_from = byte()
    if typed_from < 0x50:
        typed = None  # a known place's name, below
    elif typed_from < 0xA0:
        typed = b''
    else:
        typed = letters(1 + byte() % 8)
    known = byte() % 32
    f = byte()
    lst = 0xFFFF if f < 0x30 else 0x8000 | byte() if f < 0x40 else byte() % 32
    return op, w, dict(carry=carry, days=days, pick=pick, typed=typed, known=known, list=lst)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    a.w16(0x9030, FRAME)
    a.w16(0xA8FE, 0)
    a.w16(0xA900, SCREEN)
    a.w16(0x6AC9, MARKS)
    a.w16(0x6ACB, MARKS)
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
    RET, RETF = b'\xc3', b'\xcb'

    def stub(seg, off, far, fn=None):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), RETF if far else RET)
        if fn:
            lin = a.lin(LOAD_SEG + seg, off)
            mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def cstr(seg, off):
        out = bytearray()
        while len(out) < 0x400:
            c = mu.mem_read(a.lin(seg, (off + len(out)) & 0xFFFF), 1)[0]
            if c == 0:
                break
            out.append(c)
        return bytes(out)

    def setcf(v):
        f = reg('eflags')
        wreg('eflags', (f | 1) if v else (f & ~1))

    ans = lambda k: state['answers'][k]

    def screen():
        log(2)
        setcf(ans('carry'))

    def days():
        d, km, mins = ans('days')
        a.w16(0x1927, d)
        a.w16(0x3FF9, km)
        mu.mem_write(a.lin(a.ds, 0x3FF2), struct.pack('<I', mins & 0xFFFFFFFF))
        wreg('ax', d)
        log(5)

    def pick():
        log(9, reg('ax'))
        wreg('ax', ans('pick'))

    def typed():
        log(11)
        t = state['typed']
        mu.mem_write(a.lin(a.ds, 0x9C62), t + b'\0')

    def lst():
        log(12, reg('si'), reg('cx'))
        wreg('ax', ans('list'))

    stub(0x1194, 0x59C8, True, lambda: log(1, reg('si'), fnv(cstr(reg('ds'), reg('si'))) & 0xFFFFFFFF))
    stub(0, 0x5E3E, True, lambda: log(3))
    stub(0x292B, 0x0362, True, lambda: log(4, reg('ax')))
    stub(0x1194, 0x2E69, True, days)
    stub(0x1194, 0x4D74, True, lambda: log(6, reg('bx')))
    stub(0x1194, 0x2E14, True, lambda: log(7, reg('ax') & 0xFF))
    stub(0, 0x5C86, True, lambda: log(8, reg('bx')))
    stub(0x1194, 0x5C30, True, typed)
    stub(0x1194, 0x5730, True, lst)
    for off in (0x9A98, 0x8FC4, 0x9AE6, 0x756C, 0x9AF8, 0x7428, 0xA611, 0xA602, 0xA5CD, 0xA6B5, 0x9426,
                0x9466, 0x7530, 0x9B4E, 0x9E06, 0x9D84, 0x5828, 0x710E):
        stub(0, off, True)
    stub(0, 0x66F2, True, lambda: wreg('ax', 0x30))
    stub(0x1194, 0x5E1D, True)
    # The pick and the journey stubbed where a click or the search calls
    # them (their own vectors are travelmap's and op 3).
    pick_lin = a.lin(LOAD_SEG + 0x26F7, 0x03C4)
    jour_lin = a.lin(LOAD_SEG + 0x26F7, 0x0483)
    orig_pick = bytes(mu.mem_read(pick_lin, 1))
    orig_jour = bytes(mu.mem_read(jour_lin, 1))
    # The screen stubbed only where the button calls it (op 0).
    scr_lin = a.lin(LOAD_SEG + 0x26F7, 0x0000)
    orig_scr = bytes(mu.mem_read(scr_lin, 1))

    def scr_hook(mu, addr, size, _):
        if state.get('op') == 0:
            screen()
    mu.hook_add(UC_HOOK_CODE, scr_hook, None, scr_lin, scr_lin)

    def hook_once(lin, fn):
        def h(mu, addr, size, _):
            if state.get('stub_inner'):
                fn()
        mu.hook_add(UC_HOOK_CODE, h, None, lin, lin)
    hook_once(pick_lin, pick)
    hook_once(jour_lin, lambda: log(10))
    snap_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0625)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), snap_ds)
        op, writes, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        t = answers['typed']
        if t is None:
            k = answers['known']
            rec = 0x1445 + k * 0x19
            t = cstr(a.ds, rec).lower()
        state.update(log=[], answers=answers, typed=t, stub_inner=op in (4, 5), op=op)
        mu.mem_write(scr_lin, RETF if op == 0 else orig_scr)
        inner = op in (4, 5)
        mu.mem_write(pick_lin, RET if inner else orig_pick)
        mu.mem_write(jour_lin, RET if inner else orig_jour)
        # Unicorn keeps translated blocks: the patched bytes are seen only
        # once their blocks are dropped.
        for lin in (scr_lin, pick_lin, jour_lin):
            mu.ctl_remove_cache(lin, lin + 1)
        before = a.seg_bytes(a.ds)
        extra = ''
        try:
            for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
                wreg(r, 0)
            if op == 0:
                a.call_near(0x2542, 0x03DC)
            elif op == 1:
                a.run_range(0x26F7, 0x0000, 0x0048)
                a.run_range(0x26F7, 0x008C, 0x009B)
            elif op == 2:
                kept = (a.r16(0x0A4C) & 0xFF, a.r16(0x0A47) & 0xFF, a.r16(0x0A46) & 0xFF)
                # The location kept at the start: as it is, or another.
                if answers['carry']:
                    k = answers['known']
                    kept = (kept[0] if k & 1 else k, kept[1] if k & 2 else answers['pick'] & 3,
                            kept[2] if k & 4 else answers['list'] % 9)
                a.run_range(0x26F7, 0x00CE, 0x013D, ax=kept[0], bx=kept[1], cx=kept[2])
                extra = f' {reg("eflags") & 1}'
            elif op == 3:
                a.call_near(0x26F7, 0x0483)
            elif op == 4:
                a.call_far(0x26F7, 0x0625)
            else:
                a.call_far(0x26F7, 0x03A4)
            if a.fault is not None:
                raise Fault(a.fault)
            after = a.seg_bytes(a.ds)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before, after)
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
            digests = [fnv(changes(before, after)),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests) + extra
        except (Fault, AssertionError, UcError):
            res = 'stuck'
        out.write(f'mapscreen | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
