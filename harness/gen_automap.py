"""Emit test vectors for the automap and the levels' kept states
(src/automap.rs) from the original code.

Usage: gen_automap.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/automap.rs) an
operation and the state it reads, then runs it with EMS on (bank 11h
the map's) and its files in memory (`MemDos`: opened read/write, made,
read, written, sought, closed, deleted):

  op 0  0000:386F, the location's key (EAX)
  op 1  0000:348D (3491), the squares around the player marked
  op 2  0000:3503, the area's automap loaded (AUTOMAP.nn: none, of another
        size, or 16 records with the area's key among them or not)
  op 3  23F1:013C, the automap written back
  op 4  292B:09AC, a main quest dungeon's level's locks and triggers
        loaded (STATES.nn), or 292B:095E, kept

  automap | seed | result

Results: "fault" / "hang", or the FNV-1a digests of EAX (op 0) or the
changed DGROUP bytes (A90A unrelocated), the frame's first 5000h bytes
and the two files (AUTOMAP.nn, STATES.nn; 0 none).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_EAX

FRAME = 0x6000
WSEG = 0x8000


class MemDos(DosArena):
    """DosArena whose files are byte arrays held here (`self.files`, by
    upper-case name)."""

    def __init__(self, *a):
        super().__init__(*a)
        self.files = {}
        self.mh = {}

    def _name(self):
        lin = self.lin(self._reg('ds'), self._reg('dx'))
        raw = bytes(self.mu.mem_read(lin, 128)).split(b'\0')[0].decode('latin1')
        return raw.replace(':', '\\').split('\\')[-1].upper()

    def _new_handle(self):
        return self.new_handle(None, taken=self.mh)

    def _dos(self):
        ah = self._reg('ax') >> 8
        if ah in (0x3D, 0x3C):
            name = self._name()
            if ah == 0x3C:
                self.files[name] = bytearray()
            elif name not in self.files:
                self._wreg('ax', 2)
                self._set_cf(True)
                return True
            h = self._new_handle()
            self.mh[h] = [name, 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        if ah == 0x41:
            name = self._name()
            if name in self.files:
                del self.files[name]
                self._set_cf(False)
            else:
                self._wreg('ax', 2)
                self._set_cf(True)
            return True
        h = self._reg('bx')
        if h not in self.mh:
            return super()._dos()
        name, pos = self.mh[h]
        f = self.files[name]
        if ah == 0x3F:
            data = bytes(f[pos:pos + self._reg('cx')])
            self.mu.mem_write(self.lin(self._reg('ds'), self._reg('dx')), data)
            self.mh[h][1] = pos + len(data)
            self._wreg('ax', len(data))
        elif ah == 0x40:
            n = self._reg('cx')
            data = bytes(self.mu.mem_read(self.lin(self._reg('ds'), self._reg('dx')), n))
            if pos > len(f):
                f.extend(bytes(pos - len(f)))
            f[pos:pos + n] = data
            self.mh[h][1] = pos + n
            self._wreg('ax', n)
        elif ah == 0x42:
            off = (self._reg('cx') << 16) | self._reg('dx')
            base = {0: 0, 1: pos, 2: len(f)}[self._reg('ax') & 0xFF]
            p = (base + off) & 0xFFFFFFFF
            self.mh[h][1] = p
            self._wreg('ax', p & 0xFFFF)
            self._wreg('dx', p >> 16)
        elif ah == 0x3E:
            del self.mh[h]
        else:
            return False
        self._set_cf(False)
        return True


def digits(n):
    return bytes([0x30 + n // 10, 0x30 + n % 10])


def setup(seed):
    """(op, k, rec, slot, writes, files): writes are (segment or None for
    DGROUP, offset, bytes), files {name: bytes}; for op 2 with k 3 the
    record `rec` of AUTOMAP.<slot> takes the area's key. Mirrored in
    tests/automap.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    blob = lambda n: bytes(byte() for _ in range(n))
    op = byte() % 5
    w = []
    w.append((None, 0x05E4, pb(1 if byte() < 0x20 else byte() & 0xFE)))
    w.append((None, 0x060C, pb(byte())))
    w.append((None, 0x060E, pb(byte())))
    w.append((None, 0x0610, pk((0x1FFF, 0x2000)[byte() & 1] if byte() < 0x40 else word())))
    w.append((None, 0x0612, pk((0x1FFF, 0x2000)[byte() & 1] if byte() < 0x40 else word())))
    w.append((None, 0x837A, pk(word())))
    w.append((None, 0x0A4C, pb(byte() % 2 if byte() < 0x80 else byte())))
    w.append((None, 0x0A47, pb(3 if byte() < 0x80 else byte() % 5)))
    w.append((None, 0x0A46, pb(byte() % 9)))
    w.append((None, 0xBB0A, pb(byte() % 4 if byte() < 0xC0 else byte())))
    w.append((None, 0xC398, pk(word() & ~4 | (4 if byte() < 0x80 else 0))))
    w.append((None, 0xC39A, pk(word() & ~0x200 | (0x200 if byte() < 0x40 else 0))))
    w.append((None, 0x05E6, pb(byte() % 5)))
    w.append((None, 0x05C8, pk(word() & ~0x4000 | (0x4000 if byte() < 0x40 else 0))))
    slot = 1 + byte() % 19
    w.append((None, 0x1A6C, pb(slot)))
    w.append((None, 0x05DA, pb(byte() % 0x14)))
    w.append((None, 0x6636, pb((0, 1, byte())[byte() % 3])))
    rec = byte() % 16
    w.append((None, 0x66C6, struct.pack('<I', rec * 0x2004 if byte() < 0xE0 else word())))
    w.append((None, 0xC3D0, pk(word())))
    w.append((None, 0xC3D4, pk(word())))
    sd = byte() % 20
    w.append((None, 0x7D0D, digits(sd)))
    ad = slot if op == 2 else byte() % 20
    w.append((None, 0x6643, digits(ad)))
    w.append((WSEG, 0, blob(0x200)))
    cells = [(0, 7, 0xE, 0x15, byte())[byte() % 5] for _ in range(0x100)]
    # The map's bank (11h) mapped into the frame from the start.
    w.append((None, 0x8FA8, pk(0x44)))
    w.append((FRAME, 0, bytes(cells) * 0x40))
    for _ in range(byte() % 16):
        w.append((None, 0xC624 + byte() % 0xC0, pb(byte())))
    for _ in range(byte() % 16):
        w.append((None, 0xC766 + byte(), pb(byte())))
    w.append((None, 0xC866, pb(byte())))
    w.append((None, 0xC6E4, pb(byte())))
    files = {}
    # The automap: none (op 2 only), of another size, or 16 records.
    k = byte() % 4
    if k == 0 and op != 2:
        k = 2
    am = 'AUTOMAP.' + digits(ad).decode()
    if k == 1:
        files[am] = blob(byte() + 1) if byte() < 0xC0 else bytes(0x20040) + blob(1 + byte() % 16)
    elif k >= 2:
        f = bytearray(0x20040)
        for i in range(16):
            f[i * 0x2004:i * 0x2004 + 4] = struct.pack('<I', word() | word() << 16)
        f[rec * 0x2004 + 0x1004:rec * 0x2004 + 0x1004 + 0x100] = blob(0x100)
        files[am] = bytes(f)
    if byte() < 0xC0 or op == 4:
        files['STATES.' + digits(sd).decode()] = blob(0x1C2 * (1 + byte() % 80))
    return op, k, rec, slot, w, files


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MemDos(EXE, os.path.dirname(EXE))
    a.enable_ems(FRAME)
    a.w16(0xD806, WSEG)
    base = a.snapshot()
    rng = random.Random(0x1994_3503)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mh = {}
        op, k, rec, slot, writes, files = setup(seed)
        for seg, off, data in writes:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        a.ems['cur'] = 0x11
        a.files = {n_: bytearray(d) for n_, d in files.items()}
        if op == 2 and k == 3:
            a.call_far(0, 0x386F)
            key = a.mu.reg_read(UC_X86_REG_EAX)
            f = a.files['AUTOMAP.' + digits(slot).decode()]
            f[rec * 0x2004:rec * 0x2004 + 4] = struct.pack('<I', key)
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                a.call_far(0, 0x386F)
                res = f'{a.mu.reg_read(UC_X86_REG_EAX):x}'
            else:
                if op == 1:
                    a.call_far(0, 0x348D)
                elif op == 2:
                    a.call_far(0, 0x3503)
                elif op == 3:
                    a.call_far(0x23F1, 0x013C)
                else:
                    a.call_far(0x292B, 0x09AC if seed & 1 else 0x095E)
                if a.r16(0xA90A) == a.ds:
                    a.w16(0xA90A, a.ds - LOAD_SEG)
                after = a.seg_bytes(a.ds)
                parts = [fnv(changes(before, after)), fnv(a.mu.mem_read(a.lin(FRAME, 0), 0x5000))]
                for prefix in ('AUTOMAP.', 'STATES.'):
                    names = sorted(x for x in a.files if x.startswith(prefix))
                    parts.append(fnv(b''.join(x.encode() + bytes(a.files[x]) for x in names)) if names else 0)
                res = ' '.join(f'{p:x}' for p in parts)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'automap | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
