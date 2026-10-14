"""Emit test vectors for the loot window (src/loot_window.rs
`loot_window`, 2433:0583 through 1194:7B16) from the original code.

Usage: gen_lootwin.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/lootwin.rs) one of
four kinds of click: a key (AH 3), the staff's piece (AH 0Fh with the
staff's flat), a corpse (AH 0 or 1, its record in CSEG) or the piles of
treasure at a square (AH 0Fh with another flat; the table in EMS bank 6
at A000). The player's items, strength and gold, and the answers the
front end gives are drawn too.

Everything the port leaves to `LootHooks` is a stub: a code hook at its
first instruction, patched to RET or RETF, logs the call and gives the
scripted answer:

  1 message box (SI)     1194:59CC        2 text ([137A], AX) 1194:A374
  3 sound (SI)           23BA:012B        4 list (SI, CX) AX  1194:5734
  5 piece told CF        1194:D363        6 last piece        1194:D986
  7 piles made           2433:0000

The fonts (0000:9AE6, 9AF8) return at once. EMS is on with 20h banks,
bank 0 mapped into the frame. DGROUP's relocated words are put back to
image-relative values, as in the port (the names are read through
them).

  lootwin | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes (the segments stored there, 61C and 12C2, put back
image-relative when they are DGROUP's), of
the corpse segment's, the map and floor planes' changes, of bank 6 and
of the frame as the program sees them, and of the log.
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

FSEG = 0x5C00
CSEG = 0x6C00
WSEG = 0x7C00
FRAME = 0xA000


def item(byte):
    """A random item (19 bytes): armour, weapon or stack, occupied."""
    b = bytearray(19)
    k = byte() % 4
    if k == 0:
        b[0] = 0x80 | (byte() % 0x10)
    else:
        b[0] = byte() % 0x12
    cat = (0, 1, 3)[byte() % 3]
    # The weight word (+1), its bytes swapped: usually a light item; a
    # stack's count (the same word) now and then at 7FFFh.
    b[1] = 0 if byte() < 0xC0 else byte()
    b[2] = byte() % 0x10
    if cat == 3 and byte() < 0x20:
        b[1], b[2] = 0xFF, 0x7F
    elif byte() < 0x08:
        # The weights 1194:4D99 takes as none.
        b[1], b[2] = (0x40, 0x80)[byte() & 1], 0
    for i in range(3, 0xE):
        b[i] = byte()
    b[0xE] = 0x40 | cat << 2 | (byte() & 0x33)
    b[0xF] = 0xFF if byte() < 0x80 else byte() % 9
    b[0x10] = byte() % 8
    b[0x11] = byte()
    b[0x12] = byte()
    return bytes(b)


def items(byte, n_max):
    """40 slots: an occupied prefix of up to `n_max`, then sometimes one
    more after a gap."""
    n = byte() % (n_max + 1)
    out = bytearray()
    for i in range(40):
        out += item(byte) if i < n else bytes(19)
    if n < 39 and byte() < 0x40:
        it = item(byte)
        at = (n + 1) * 19
        out[at:at + 19] = it
    return bytes(out)


def setup(seed):
    """(writes, bank 6 bytes, (ax, si, seg, bx), answers). Writes are
    (segment name, offset, bytes); mirrored in tests/lootwin.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    kind = byte() % 4
    w.append(('ds', 0x0212, items(byte, 40)))
    # The strength: now and then one whose s (1194:5B07) rounds at 80h.
    w.append(('ds', 0x01CD, bytes([(32, 96, 160, 224)[byte() % 4] if byte() < 0x40 else byte()])))
    w.append(('ds', 0x05C2, pk(word()) + pk(word())))
    w.append(('ds', 0x05BD, bytes([byte()])))
    w.append(('ds', 0x4D77, pk(byte() % 0x24)))
    w.append(('ds', 0x4BF4, bytes([byte()])))
    w.append(('ds', 0xC39A, pk(word() & ~0x200 | (0x200 if byte() < 0x30 else 0))))
    w.append(('ds', 0xC398, pk(word())))
    w.append(('ds', 0x05E6, bytes([4 if byte() < 0xC0 else byte() % 6])))
    w.append(('ds', 0x0D48, bytes([byte() % 12])))
    w.append(('ds', 0xBB0A, bytes([byte() % 4])))
    bank6 = bytearray(0x10000)
    si, seg, bx = word(), 'ds', word()
    if kind == 0:
        al = byte() % 30
        ah = 3
        keys = bytes(0xC5 if byte() < 0x60 else byte() % 30 for _ in range(12))
        w.append(('ds', 0x0652, keys))
    elif kind == 1:
        staff = byte()
        al, ah = staff, 0x0F
        w.append(('ds', 0xC941, bytes([staff])))
        w.append(('ds', 0x0F77, bytes([byte() % 10])))
        w.append(('ds', 0x0F7D, bytes([byte() & 1])))
        bx = word()
        w.append(('map', bx, pk(word())))
        w.append(('floor', bx, bytes([0 if byte() < 0x80 else byte()])))
    elif kind == 2:
        ah = byte() & 1
        al = byte()
        si = (byte() % 0x30) * 0x100
        seg = 'c'
        head = bytes(byte() for _ in range(0x6E))
        w.append(('c', si, head))
        w.append(('c', si + 0x6E, items(byte, 40 if byte() < 0x10 else 8)))
        w.append(('c', si + 0x407, pk(0 if byte() < 0x80 else word())))
        w.append(('c', si + 0x40D, pk(word())))
    else:
        al = byte()
        ah = 0x0F
        w.append(('ds', 0xC941, bytes([al ^ (1 + byte() % 0xFF)])))
        bx = word() & 0x7FFE
        count = byte() % 12
        w.append(('ds', 0x0FF2, bytes([count])))
        level = byte() % 4
        place = byte() % 6
        for i in range(count + byte() % 3):
            e = bytearray(0x1C)
            e[0] = 1
            e[1] = place if byte() < 0xC0 else byte() % 6
            sq = bx if byte() < 0xC0 else word() & 0x7FFE
            e[2:4] = pk(sq)
            e[4] = level if byte() < 0xC0 else byte() % 4
            g = byte()
            e[5:7] = pk(0xFFFF if g < 0x90 else 1 if g < 0x98 else word() % 300)
            e[7] = 0xFF if byte() < 0xC0 else byte() % 10
            e[8] = byte()
            e[9:0x1C] = item(byte)
            if byte() < 0x20:
                e[0x13:0x17] = bytes(4)
            bank6[0xA000 + 0x1C * i:0xA000 + 0x1C * (i + 1)] = e
        if count > 0 and byte() < 0x40:
            # The first pile a stack here, and the player's first item a
            # stack of its type at 7FFFh: taking it overflows.
            e = bank6
            e[0xA001] = place
            e[0xA002:0xA004] = pk(bx)
            e[0xA004] = level
            e[0xA005:0xA007] = pk(0xFFFF)
            e[0xA007] = 0xFF
            e[0xA009 + 0xE] |= 0x0C
            it = bytearray(e[0xA009:0xA01C])
            it[1], it[2] = 0xFF, 0x7F
            it[0xE] = (it[0xE] | 0x4C) & 0xFD
            w.append(('ds', 0x0212, bytes(it)))
        w.append(('ds', 0xBB0A, bytes([level])))
        w.append(('map', bx, pk(word() & 0x0FFF | (0x8000 if byte() < 0x80 else 0))))
        w.append(('floor', bx, bytes([byte()])))
    lists = [0xFFFF if byte() < 0x30 else byte() % 12 for _ in range(12)]
    cfs = [byte() & 1 for _ in range(4)]
    return w, bytes(bank6), (ah << 8 | al, si, seg, bx), (lists, cfs)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    # The INT 3s the window runs in play (2433:0298, a pile with neither
    # gold nor name; 0883's past the piles in use): DOS ignores them.
    a.GAME_INT3 = a.GAME_INT3 | {(0x2433, 0x0299), (0x2433, 0x08B1)}
    # DGROUP's relocated words back to image-relative values (gen_items).
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    RETF, RET = b'\xcb', b'\xc3'
    for seg, off, code in [(0, 0x9AE6, RETF), (0, 0x9AF8, RETF)]:
        a.patch(seg, off, code)
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    wreg = lambda r, v: mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def carry(cf):
        f = reg('eflags')
        wreg('eflags', f | 1 if cf else f & ~1)

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

    def lst():
        # A pick among the list's entries (the list returns no other).
        log(4, reg('si'), reg('cx'))
        v = take(0, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(reg('cx'), 1))

    def told():
        log(5)
        carry(take(1, 1))

    stub(0x1194, 0x59CC, False, lambda: log(1, reg('si')))
    stub(0x1194, 0xA374, False, lambda: log(2, a.r16(0x137A), reg('ax')))
    stub(0x23BA, 0x012B, True, lambda: log(3, reg('si')))
    stub(0x1194, 0x5734, False, lst)
    stub(0x1194, 0xD363, True, told)
    stub(0x1194, 0xD986, True, lambda: log(6))
    stub(0x2433, 0x0000, True, lambda: log(7))
    if os.environ.get('TRACE'):
        for off in (0x08A5, 0x08B1, 0x08BB, 0x08C6):
            lin = a.lin(LOAD_SEG + 0x2433, off)
            mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, o: print(f'T {o:04x} si {reg("si"):04x} cx {reg("cx"):04x} dx {reg("dx"):04x} bx {reg("bx"):04x} ax {reg("ax"):04x}', file=sys.stderr), off, lin, lin)
    cover = set()
    if os.environ.get('COVER'):
        for seg, lo, hi in ((0x2433, 0x0583, 0x0C75), (0x2433, 0x0152, 0x02A0), (0x1194, 0x4D99, 0x4E54), (0x1194, 0x5ED1, 0x5F13)):
            l0 = a.lin(LOAD_SEG + seg, 0)
            mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, d: cover.add(addr - d), l0, l0 + lo, l0 + hi)
    a.enable_ems(FRAME)
    snap = a.snapshot()
    segs = {'ds': a.ds, 'c': CSEG, 'map': WSEG, 'floor': FSEG}
    rng = random.Random(0x1994_0583)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        a.w16(0xD806, WSEG)
        a.w16(0xCCD0, FSEG)
        writes, bank6, (ax, si, seg, bx), answers = setup(seed)
        for name, off, data in writes:
            head = min(len(data), 0x10000 - off)
            mu.mem_write(a.lin(segs[name], off), data[:head])
            if head < len(data):
                mu.mem_write(a.lin(segs[name], 0), data[head:])
        a.ems['banks'][6] = bank6
        state.update(log=[], answers=answers, n=[0, 0])
        before = {k: a.seg_bytes(v) for k, v in segs.items()}
        try:
            a.call_far(0x1194, 0x7B16, ax=ax, si=si, fs=segs[seg], bx=bx, es=a.ds)
            after = {k: a.seg_bytes(v) for k, v in segs.items()}
            ds = bytearray(after['ds'])
            # Segments the window stores, relocated in the emulator: the
            # message line's (61C) and the record's (12C2) when DGROUP.
            for at in (0x061C, 0x12C2):
                if struct.unpack_from('<H', ds, at)[0] == a.ds and before['ds'][at:at + 2] != ds[at:at + 2]:
                    ds[at:at + 2] = struct.pack('<H', DGROUP)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before['ds'], bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before["ds"][o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
            digests = [fnv(changes(before['ds'], bytes(ds)))]
            digests += [fnv(changes(before[k], after[k])) for k in ('c', 'map', 'floor')]
            digests += [fnv(a.ems_bank(6)), fnv(a.seg_bytes(FRAME))]
            digests.append(fnv(b''.join(struct.pack('<H', v & 0xFFFF) for v in state['log'])))
            res = ' '.join(f'{d:x}' for d in digests)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'lootwin | {seed:x} | {res}\n')
    out.close()
    if cover:
        with open(os.environ['COVER'], 'w') as f:
            for x in sorted(cover):
                f.write(f'{x:04x}\n')


if __name__ == '__main__':
    main()
