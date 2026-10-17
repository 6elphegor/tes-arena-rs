"""Emit test vectors for the equipment store (src/store.rs
`equipment_store`, 1194:F48E) from the original code.

Usage: gen_store.py ACD_unpacked.EXE GAME_DIR OUT

A case fills from a seed (`setup`, mirrored in tests/store.rs) the state
the store reads: a fight or the guards coming (FFB, 680F), the RNG's seed
(91F6), the player (gold 5C2, personality 1D3, strength 1CD, the
lockpicking bytes, class), the player's 40 item slots (212), the date
(5CC), the city's byte the shop's level reads (0A22 + 25h), the places'
list the seed and the shopkeeper's name come from (FD2, 138B, the far
pointer at 1008 to CSEG, A46), the path buffers' turn (1AEA), the map
(MAP1 at D806, WSEG); the store's files: its stock (fresh, stale, short,
or none) and its repairs; and the answers the front end gives: the
menus' picks, the lists' picks and the haggles (the price agreed, or
walked away).

GLOBAL.BSA stays closed (A920 FFFFh, as in the image), so a failed open
looks nothing up there. EMS is off; the list segment (9030) is FSEG. The
files live in memory (int 21h 3Ch, 3Dh, 3Eh, 3Fh, 40h, 41h, 42h, 4Eh, 2Fh
on them; other names go to the game directory, read only).

Everything the port leaves to `StoreHooks` is a stub: a code hook at its
first instruction, patched to RET or RETF, logs the call and gives the
scripted answer (the carry clear unless scripted):

  1 greeting (4BFF: SI)          2 expand (2DA2:000A: DI)
  3 quests (8F1E)                4 menu (020E: SI BX DI) AX
  5 list (5734: SI CX) AX, DS = ES = DGROUP as the list leaves them
  6 message (4D31: SI)           7 message box (59CC: SI)
  8 haggle (8020: DH, EAX, SI, DI) EAX, CF
  9 weapons' lines (F142)        10 armour's lines (F204)
  11 repairs (C6AC)

The screen's routines (0000:8FC4, 77EC, 9466, 541E, 9AE6, 9AF8; 1194:
5496, 54B3, 5E21, 4B60, 5F8C, 545C, 5FC0) return at once. DGROUP's
relocated words are put back to image-relative values, as in the port.

  store | seed | result

Debugging: ONLY=seed runs that case alone; DEBUG=1 says where a stuck
case stopped (with TRAIL=1, the blocks before it first left the image,
and DS's changes); WATCH=lo-hi prints the writes to DGROUP lo..hi;
DUMP=seed prints the case's changed DGROUP bytes, log and files.

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (the guards' hook segment 67EC and the open
file's name's segment A90A put back image-relative), of FSEG's and WSEG's changes, of the log, and of the
files at the end (each name, its length and bytes, in name order; and the
names deleted).
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

CSEG = 0x6C00
WSEG = 0x7C00
FSEG = 0x5C00


class StoreDos(DosArena):
    """DosArena with a few files kept in memory (by upper-case name)."""

    def __init__(self, *a):
        super().__init__(*a)
        self.files = {}
        self.deleted = []
        self.mh = {}

    def _name(self):
        lin = self.lin(self._reg('ds'), self._reg('dx'))
        raw = bytes(self.mu.mem_read(lin, 128)).split(b'\0')[0].decode('latin1')
        return raw.replace(':', '\\').split('\\')[-1].upper()

    def _handle(self):
        return self.new_handle(None, taken=self.mh)

    def _dos(self):
        ah = self._reg('ax') >> 8
        if ah == 0x3C:
            n = self._name()
            self.files[n] = bytearray()
            h = self._handle()
            self.mh[h] = [n, 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        if ah == 0x3D:
            n = self._name()
            if n not in self.files:
                if n.startswith(('EQ', 'RE')) and n.endswith('.64'):
                    self._wreg('ax', 2)
                    self._set_cf(True)
                    return True
                return super()._dos()
            h = self._handle()
            self.mh[h] = [n, 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        if ah == 0x41:
            n = self._name()
            self.files.pop(n, None)
            self.deleted.append(n)
            self._set_cf(False)
            return True
        if ah == 0x4E:
            self._set_cf(self._name() not in self.files)
            if self._name() not in self.files:
                self._wreg('ax', 2)
            return True
        if ah == 0x2F:
            self.mu.reg_write(UC_X86_REG_ES, self.ds)
            self._wreg('bx', 0x0080)
            return True
        h = self._reg('bx')
        if h in self.mh:
            name, pos = self.mh[h]
            f = self.files[name]
            if ah == 0x3F:
                n = self._reg('cx')
                data = bytes(f[pos:pos + n])
                self.mh[h][1] = pos + len(data)
                self.mu.mem_write(self.lin(self._reg('ds'), self._reg('dx')), data)
                self._wreg('ax', len(data))
                self._set_cf(False)
                return True
            if ah == 0x40:
                n = self._reg('cx')
                data = bytes(self.mu.mem_read(self.lin(self._reg('ds'), self._reg('dx')), n))
                if len(f) < pos + n:
                    f.extend(bytes(pos + n - len(f)))
                f[pos:pos + n] = data
                self.mh[h][1] = pos + n
                self._wreg('ax', n)
                self._set_cf(False)
                return True
            if ah == 0x42:
                off = (self._reg('cx') << 16) | self._reg('dx')
                base = {0: 0, 1: pos, 2: len(f)}[self._reg('ax') & 0xFF]
                p = (base + off) & 0xFFFFFFFF
                self.mh[h][1] = p
                self._wreg('ax', p)
                self._wreg('dx', p >> 16)
                self._set_cf(False)
                return True
            if ah == 0x3E:
                del self.mh[h]
                self._set_cf(False)
                return True
        if ah == 0x3E and h not in self.handles:
            # Closing a handle already closed: DOS's "invalid handle".
            self._wreg('ax', 6)
            self._set_cf(True)
            return True
        return super()._dos()


def setup(seed):
    """(writes, cseg bytes, files, answers). Mirrored in tests/store.rs,
    draw for draw (no tuple draws two)."""
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
        gold = byte() % 0x40
    elif k < 0xC0:
        gold = word() % 0x1000
    else:
        gold = byte() | byte() << 8 | byte() << 16
    w.append((0x05C2, pd(gold)))
    w.append((0x01D3, bytes([byte()])))
    # Strong half the time (so purchases aren't too heavy), and often a
    # good lockpick.
    w.append((0x01CD, bytes([0xFF if byte() < 0x80 else byte()])))
    for at in (0x01A4 + 0x2A, 0x01A4 + 0x2C):
        w.append((at, bytes([0xFF if byte() < 0x60 else byte() % 0x40])))
    w.append((0x01A4 + 0x05, bytes([byte() % 0x12])))
    w.append((0x01A4 + 0x06, bytes([byte() % 3 if byte() < 0x80 else byte() % 0x20])))
    w.append((0x01A9, bytes([byte()])))
    # The player's items: the first n occupied, the rest empty (or junk
    # that is never reached past the first empty slot).
    n = byte() % 41
    for i in range(40):
        at = 0x0212 + 0x13 * i
        rec = bytearray(0x13)
        if i < n:
            cat = byte() % 4
            if byte() < 8:
                ty = 0x80 | byte() % 8
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
            if byte() < 0x30:
                flags |= 0x80
            rec[0] = ty
            # +1: the weight (its high byte) or a stack's count; mostly light.
            rec[1:3] = pk((byte() % 6) << 8 if byte() < 0xC0 else word())
            rec[3] = byte()
            rec[4] = byte()
            rec[5] = byte()
            health = 0 if byte() < 0x20 else word() % 0x2000
            rec[6:8] = pk(health)
            rec[8:10] = pk(0 if byte() < 0x08 else word() % 0x2000 + 1)
            kk = byte()
            rec[0x0A:0x0E] = pd(0 if kk < 0x20 else word() % 0x4000 if kk < 0xE0 else dword())
            rec[0x0E] = flags
            rec[0x0F] = 0xFF if byte() < 0x80 else byte() % 0x18
            rec[0x10] = 0xFF if byte() < 0x80 else byte() % 8
            rec[0x11] = byte()
            rec[0x12] = byte()
        w.append((at, bytes(rec)))
    day = byte() % 0x168
    w.append((0x05CC, pd((byte() % 4) * 518400 + day * 1440 + word() % 1440)))
    w.append((0x0A22 + 0x25, bytes([byte() % 3])))
    w.append((0x0A46, bytes([byte() % 8])))
    w.append((0x1AEA, bytes([0xFF if byte() & 1 else 0])))
    key = word()
    w.append((0x0FD2, pk(key)))
    count = byte() % 6
    w.append((0x138B, pk(count)))
    w.append((0x1008, pk(0x10)))
    w.append((0x100A, pk(CSEG)))
    cseg = bytearray(0x200)
    for i in range(0x80):
        cseg[i] = byte()
    look = (key & 0xFF00) | ((key & 0xFF) >> 1)
    for i in range(count):
        at = 0x10 + 4 * i
        if byte() < 0x80:
            cseg[at:at + 2] = pk(look)
        else:
            cseg[at:at + 2] = pk(word())
        cseg[at + 2:at + 4] = pk(word())
    # The files, named from the seed the store will find: the first
    # entry with the key's word, else (none) the entry at 0 (the word at
    # CSEG:0), or with no entries the list's own first word.
    files = {}
    seedw = None
    for i in range(count):
        at = 0x10 + 4 * i
        if struct.unpack_from('<H', cseg, at)[0] == look:
            seedw = look
            break
    if seedw is None:
        seedw = struct.unpack_from('<H', cseg, 0x10 if count == 0 else 0)[0]
    digits = str(seedw)
    name = (digits + '.64')
    stock = 'EQ' + name[2:].upper()
    repairs = 'RE' + name[2:].upper()
    k = byte()
    if k < 0xA0:
        now = struct.unpack('<I', w[[x[0] for x in w].index(0x05CC)][1])[0]
        kk = byte()
        # A quarter at the day's edge (5A0h minutes old).
        age = 0x59F + byte() % 3 if kk < 0x40 else byte() % 0x10 if kk < 0xC0 else word()
        data = bytearray(pd(now - age))
        for _ in range(2):
            n = byte() % 12
            data.append(n)
            for _ in range(n):
                magic = 1 if byte() < 0x60 else 0
                weapon = byte() & 1
                ty = byte() % 18 if weapon else byte() % 11
                rec = bytearray(13)
                rec[0] = byte()
                rec[1] = magic
                rec[2] = weapon
                rec[3] = byte() % 3
                rec[4] = 0xFF if byte() < 0x60 else byte() % 8
                rec[5] = ty
                rec[6] = 0xFF if byte() < 0x60 else byte() % 0x10
                rec[7:11] = pd(word() % 0x2000)
                rec[11] = byte()
                rec[12] = byte()
                data.extend(rec)
        if byte() < 0x20:
            data = data[:byte() % len(data)]
        files[stock] = bytes(data)
    if byte() < 0x60:
        rep = bytearray(0x78)
        for i in range(5):
            if byte() < 0x80:
                rep[0x18 * i] = 1 + byte() % 0xFF
                for j in range(1, 0x18):
                    rep[0x18 * i + j] = byte()
        files[repairs] = bytes(rep)
    menus = []
    for _ in range(8):
        kk = byte() % 8
        menus.append(0xFFFF if kk == 7 else kk % 5)
    lists = []
    for _ in range(6):
        lists.append(0xFFFF if byte() < 0x30 else byte())
    haggles = []
    for _ in range(4):
        cf = byte() < 0x40
        kk = byte()
        if kk < 0x30:
            eax = gold
        elif kk < 0xC0:
            eax = word() % 0x800
        else:
            eax = byte() | byte() << 8 | byte() << 16
        haggles.append((cf, eax))
    # The end of the scratch the set-up clears (1044 + C8h bytes).
    w.append((0x1104, bytes(byte() for _ in range(8))))
    return w, bytes(cseg), files, (menus, lists, haggles)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
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

    def menu():
        log(4, reg('si'), reg('bx'), reg('di'))
        keys = bytes(mu.mem_read(a.lin(a.ds, reg('bx')), 8)).split(b'\0')[0]
        v = take(0, 0xFFFF)
        wreg('ax', v if v == 0xFFFF else v % max(len(keys), 1))

    def lst():
        log(5, reg('si'), reg('cx'))
        v = take(1, 0xFFFF)
        n = reg('cx')
        wreg('ax', v if v == 0xFFFF else v % max(n, 1))
        wreg('ds', a.ds)
        wreg('es', a.ds)

    def haggle():
        log(8, reg('edx') >> 8 & 0xFF, reg('eax') & 0xFFFF, reg('eax') >> 16, reg('si'), reg('di'))
        cf, eax = take(2, (True, 0))
        wreg('eax', eax)
        carry(cf)

    stub(0x1194, 0x4BFF, False, lambda: log(1, reg('si')))
    stub(0x2DA2, 0x000A, True, lambda: log(2, reg('di')))
    stub(0x1194, 0x8F1E, False, lambda: log(3))
    stub(0x1194, 0x020E, True, menu)
    stub(0x1194, 0x5734, False, lst)
    stub(0x1194, 0x4D31, False, lambda: log(6, reg('si')))
    stub(0x1194, 0x59CC, False, lambda: log(7, reg('si')))
    stub(0x1194, 0x8020, False, haggle)
    stub(0x1194, 0xF142, False, lambda: log(9))
    stub(0x1194, 0xF204, False, lambda: log(10))
    stub(0x1194, 0xC6AC, False, lambda: log(11))
    if os.environ.get('WATCH'):
        from unicorn import UC_HOOK_MEM_WRITE
        lo, hi = (int(x, 16) for x in os.environ['WATCH'].split('-'))
        def wr(mu, acc, addr, size, val, _):
            if not state.get('watched'):
                state['watched'] = True
                t = state.get('trail', [])
                print('before', ' '.join(f'{x - LOAD_SEG * 16:05x}' for x in t[-30:]), file=sys.stderr)
                print(f'regs si={reg("si"):04x} di={reg("di"):04x} cx={reg("cx"):04x} ax={reg("ax"):04x} ds={reg("ds"):04x} es={reg("es"):04x}', file=sys.stderr)
            print(f'write {addr - a.ds * 16:04x} size {size} val {val:x} at {reg("cs") - LOAD_SEG:04x}:{reg("ip"):04x}', file=sys.stderr)
        mu.hook_add(UC_HOOK_MEM_WRITE, wr, None, a.lin(a.ds, lo), a.lin(a.ds, hi))
    if os.environ.get('TRAIL'):
        from unicorn import UC_HOOK_BLOCK
        trail = state.setdefault('trail', [])
        mu.hook_add(UC_HOOK_BLOCK, lambda mu, addr, size, _: (trail.append(addr), len(trail) > 20000 and trail.pop(0)))
        dsl = state.setdefault('dsl', [])

        def dsrec(mu, addr, size, _):
            d = reg('ds')
            if not dsl or dsl[-1][1] != d:
                dsl.append((addr - LOAD_SEG * 16, d))
        mu.hook_add(UC_HOOK_BLOCK, dsrec)
    snap = a.snapshot()
    rng = random.Random(0x1994_F48E)
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
        # DOS handle 5 is GLOBAL.BSA's in the game (and the port's loose
        # files take 6); the BSA itself stays closed here.
        a.handles[5] = [b'', 0]
        a.w16(0xD806, WSEG)
        a.w16(0x9030, FSEG)
        writes, cseg, files, answers = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(CSEG, 0), cseg)
        a.files = {k: bytearray(v) for k, v in files.items()}
        menus, lists, haggles = answers
        state.get('trail', []).clear()
        state.get('dsl', []).clear()
        state.update(log=[], answers=(menus, lists, haggles), n=[0, 0, 0])
        for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp'):
            wreg(r, 0)
        wreg('eflags', 0x0002)
        before = (a.seg_bytes(a.ds), a.seg_bytes(FSEG), a.seg_bytes(WSEG))
        try:
            a.call_far(0x1194, 0xF48E)
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', ds, 0x67EC)[0] == LOAD_SEG and before[0][0x67EC:0x67EE] != ds[0x67EC:0x67EE]:
                ds[0x67EC:0x67EE] = struct.pack('<H', 0)
            # The last open file's name's segment (A90A): DGROUP, relocated.
            if struct.unpack_from('<H', ds, 0xA90A)[0] == a.ds:
                ds[0xA90A:0xA90C] = struct.pack('<H', DGROUP)
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before[0], bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[0][o]:02x} {v:02x}', file=sys.stderr)
                print('log', state['log'], file=sys.stderr)
                print('files', {k: bytes(v).hex() for k, v in a.files.items()}, a.deleted, file=sys.stderr)
            fl = b''.join(k.encode() + b'\0' + struct.pack('<I', len(v)) + bytes(v) for k, v in sorted(a.files.items()))
            fl += b''.join(k.encode() + b'\0' for k in a.deleted)
            digests = [fnv(changes(before[0], bytes(ds))), fnv(changes(before[1], a.seg_bytes(FSEG))),
                       fnv(changes(before[2], a.seg_bytes(WSEG))),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log'])), fnv(fl)]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                cs, ip = reg('cs') - LOAD_SEG, reg('ip')
                print(f'{seed:x}: {e!r} at {cs:04x}:{ip:04x} log {state["log"][:40]}', file=sys.stderr)
                if 'trail' in state:
                    t = state['trail']
                    out_at = next((i for i, x in enumerate(t) if not (LOAD_SEG * 16 <= x < (LOAD_SEG + 0x3BA0) * 16 or x == 0x80000)), len(t))
                    print(' '.join(f'{x - LOAD_SEG * 16:05x}' for x in t[max(0, out_at - 40):out_at + 3]), file=sys.stderr)
                    print('ds changes', ' '.join(f'{x:05x}:{d:04x}' for x, d in state['dsl'][-30:]), file=sys.stderr)
        out.write(f'store | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
