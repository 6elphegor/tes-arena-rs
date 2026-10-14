"""Emit test vectors for the wilderness's run (src/wild.rs): its set-up
`wild_setup` (217B:0084), the chunk shift `wild_shift` (217B:0265), the
leaving `wild_done` (217B:04DC), a city's gate out `wild_leave_city`
(217B:027A) and the packer `wild_compress` (2294:0D03).

Usage: gen_wildrun.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/wildrun.rs) the
state the routine reads; EMS is off. Stubbed: 0000:0F05 (the weather;
counts at DGROUP FFE0), 217B:0189 (a block loaded; logged: AL the slot,
AH the block) and 0000:9DD6 (a file written; logged: the name, CX and
the FNV-1a of the bytes at ES:DI; AX 1).

  wildrun | seed | op | result

Ops: setup, shift, done, leave, pack. Results: "fault" / "hang", or
digests: the changed DGROUP bytes, the buffer segment (64 KiB), the
walls' segment (64 KiB) and the log; for pack AX and the 400h bytes at
the destination. The leave cases run with a cap of 400M instructions.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, changes, fnv, mine
from gen_attack import stream
from gen_frame import counter
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_CX, UC_X86_REG_SI, UC_X86_REG_ES, UC_X86_REG_DI, \
    UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SS, UC_X86_REG_SP, UC_X86_REG_DS, UC_X86_REG_BX

OPS = ['setup', 'shift', 'done', 'leave', 'pack']
BUF, FLOOR, WALLS, CEIL = 0x7000, 0xA000, 0xB000, 0xC000
SRC, DST = 0xD000, 0xE000
TEMPLATES = [b'city1.mif', b'cityw1.mif', b'cityw3.mif', b'townw1.mif', b'town2.mif', b'villagw4.mif',
             b'village3.mif', b'imperial.mif']


def setup(seed, op):
    """(DGROUP writes, other writes as (seg, off, bytes), extra). Mirrored
    in tests/wildrun.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w, o = [], []
    edge = lambda: (0x0FFF, 0x1000, 0x2FFF, 0x3000, 0x3001, 0x1FFF, 0x2000, 0, 0x3FFF)[byte() % 9]
    pos = lambda: edge() if byte() < 0x80 else (word() & 0x3FFF if byte() < 0xC0 else word())
    if op == 'setup':
        w.append((0xC39A, pk(word() & ~0x2000 | (0x2000 if byte() < 0x60 else 0))))
        w.append((0x0A22, struct.pack('<I', word() | word() << 16)))
        t = TEMPLATES[byte() % len(TEMPLATES)]
        w.append((0x0A36, t + b'\0'))
        w.append((0x0616, struct.pack('<I', word() | word() << 16)))
        for at in (0x060C, 0x060E):
            w.append((at, pk(byte() % 0x40 if byte() < 0xC0 else word())))
        for at in (0x0610, 0x0612, 0x0614, 0xC3DA):
            w.append((at, pk(pos())))
        for at in (0xC3D0, 0xC3D4):
            w.append((at, pk(pos())))
        cb = word()
        d = (0x80, 0x81, 0x100, 0x101, 0xFF, 0x7F, 0x8000, 0, 0x80, 0x180)[byte() % 10]
        d = d if byte() & 1 else (-d) & 0xFFFF
        fd = (cb - d) & 0xFFFF if byte() < 0xC0 else word()
        w.append((0xCB90, pk(cb)))
        w.append((0x0FD2, pk(fd)))
        w.append((0x05C8, pk(word())))
    elif op == 'shift':
        w.append((0xACC2, bytes([0 if byte() < 0xE0 else byte()])))
        for at in (0xC3D0, 0xC3D4, 0x0600, 0x0602):
            w.append((at, pk(pos())))
        for at in (0x0608, 0x060A):
            w.append((at, pk(byte() % 0x40 if byte() < 0xE0 else word())))
        w.append((0x05C8, pk(word())))
        w.append((0x05CA, pk(word())))
        w.append((0x0604, struct.pack('<I', word() | word() << 16)))
        for i in range(23):
            w.append((0x0AB3 + i * 0x1C, pk(word()) + pk(word()) + pk(word())))
        grid = bytes(byte() % 8 for _ in range(0x100))
        k = word()
        o.append((BUF, 0xF000, (grid * 16)[k % 0x100:] + (grid * 16)[:k % 0x100]))
        # The window as loaded now and then (no change).
        if byte() < 0x60:
            x, y = 0, 0
            for at, v in w:
                if at == 0x0608: x = struct.unpack('<H', v)[0]
                if at == 0x060A: y = struct.unpack('<H', v)[0]
            # Mirrored in the test by reading the grid after writing.
            return w, o, ('same', x, y)
    elif op == 'done':
        for at in (0xC3D0, 0xC3D4, 0xC3DA, 0x0608, 0x060A):
            w.append((at, pk(word())))
        n = byte() % 12
        w.append((0xC2B5, bytes(1 + byte() % 0xFF for _ in range(n)) + b'\0'))
    elif op == 'leave':
        w.append((0xC357, pk((0x80, 0x80, 0x40, 0x7F, 0x81, 0)[byte() % 6] if byte() < 0xE0 else word() & 0xFF)))
        w.append((0xC359, pk((0x80, 0x80, 0x40, 0x7F, 0x81, 0)[byte() % 6] if byte() < 0xE0 else word() & 0xFF)))
        w.append((0x902E, pk(0 if byte() < 0xC0 else word() & 0x7FFE)))
        w.append((0x1AEA, bytes([byte() & 1])))
        vals = (0x2F2F, 0x2D2D, 0x2E2E, 0xA3A3, 0xB3B3, 0x0000, 0x8080, 0x2F2E, 0x3030, 0x0101)
        walls = bytearray()
        for _ in range(0x2000):
            v = vals[byte() % 10] if byte() < 0xC0 else word()
            walls += pk(v)
        # 16 KiB of words, four times: the walls plane (32 KiB) and what
        # lies past it in its segment (the clear can run on into it).
        o.append((WALLS, 0, bytes(walls) * 4))
        ceil = bytes(byte() for _ in range(0x100))
        o.append((CEIL, 0, ceil * 0x80))
        flo = bytes(byte() for _ in range(0x100))
        o.append((FLOOR, 0, flo * 0x80))
        # The buffer segment, as the heap gives it (any low bits).
        w.append((0x9030, pk(BUF + byte() % 16)))
    else:  # pack
        n = (0, 1, 2, 3, 4, 6, 8, 0x20, 0x41)[byte() % 9] if byte() < 0x80 else byte() * 2 + (byte() & 1 if byte() < 0x30 else 0)
        alpha = [word() for _ in range(1 + byte() % 4)]
        data = bytearray()
        while len(data) < n + 4:
            v = alpha[byte() % len(alpha)]
            data += pk(v) * (1 + (byte() % 5 if byte() < 0xA0 else 0))
        o.append((SRC, 0, bytes(data[:0x200])))
        return w, o, ('pack', n, word())
    return w, o, None


def call_far_long(a, seg, off):
    """Arena.call_far with a cap of 400M instructions."""
    mu = a.mu
    ss, sp = 0x9000, 0xfff0 - 4
    mu.mem_write(a.lin(ss, sp), struct.pack('<HH', 0, STUB_SEG))
    for r, v in ((UC_X86_REG_DS, a.ds), (UC_X86_REG_ES, a.ds), (UC_X86_REG_SS, ss), (UC_X86_REG_SP, sp),
                 (UC_X86_REG_CS, LOAD_SEG + seg), (UC_X86_REG_IP, off)):
        mu.reg_write(r, v)
    a.fault = None
    mu.emu_start(a.lin(LOAD_SEG + seg, off), a.lin(STUB_SEG, 0), count=400_000_000)
    if a.fault is not None:
        raise Fault(a.fault)
    assert mu.reg_read(UC_X86_REG_CS) == STUB_SEG, 'did not return'


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG, 0x0F05), counter(0xFFE0, True))
    a.mu.mem_write(a.lin(LOAD_SEG + 0x217B, 0x0189), b'\xc3')
    a.mu.mem_write(a.lin(LOAD_SEG, 0x9DD6), b'\xb8\x01\x00\xf8\xc3')
    for var, seg in ((0x9030, BUF), (0xCCD0, FLOOR), (0xD806, WALLS), (0xD748, CEIL)):
        a.w16(var, seg)
    a.w16(0x91F0, 0)
    log = []

    def loads(mu, addr, size, _):
        log.append(b'L' + struct.pack('<H', mu.reg_read(UC_X86_REG_AX)))

    def writes(mu, addr, size, _):
        si, cx = mu.reg_read(UC_X86_REG_SI), mu.reg_read(UC_X86_REG_CX)
        es, di = mu.reg_read(UC_X86_REG_ES), mu.reg_read(UC_X86_REG_DI)
        name = bytes(mu.mem_read(a.lin(a.ds, si), 0x50)).split(b'\0')[0]
        data = bytes(mu.mem_read(a.lin(es, di), cx))
        log.append(b'W' + name + b'\0' + struct.pack('<HQ', cx, fnv(data)))

    a.mu.hook_add(UC_HOOK_CODE, loads, begin=a.lin(LOAD_SEG + 0x217B, 0x0189), end=a.lin(LOAD_SEG + 0x217B, 0x0189))
    a.mu.hook_add(UC_HOOK_CODE, writes, begin=a.lin(LOAD_SEG, 0x9DD6), end=a.lin(LOAD_SEG, 0x9DD6))
    BASE = bytes(a.mu.mem_read(0, 0x110000))
    rng = random.Random(0x1994_0084)
    n_each = int(os.environ.get('N', 800))
    total = n_each * len(OPS)
    for n in range(total):
        op = OPS[n % len(OPS)]
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(0, BASE)
        log.clear()
        w, o, extra = setup(seed, op)
        for at, data in w:
            a.mu.mem_write(a.lin(a.ds, at), data)
        for seg, off, data in o:
            a.mu.mem_write(a.lin(seg, off), data)
        if extra and extra[0] == 'same':
            x, y = extra[1], extra[2]
            at = ((y << 6) + x + 0xF000) & 0xFFFF
            g = bytes(a.mu.mem_read(a.lin(BUF, at), 2)) + bytes(a.mu.mem_read(a.lin(BUF, (at + 0x40) & 0xFFFF), 2))
            a.mu.mem_write(a.lin(a.ds, 0x0604), g)
        before = a.seg_bytes(a.ds)
        try:
            if op == 'pack':
                _, length, bx = extra
                mu = a.mu
                sp = 0xFFF0 - 14
                mu.mem_write(a.lin(0x9000, sp), struct.pack('<7H', 0, STUB_SEG, 0, SRC, 0, DST, length))
                for r, v in ((UC_X86_REG_DS, a.ds), (UC_X86_REG_ES, a.ds), (UC_X86_REG_SS, 0x9000), (UC_X86_REG_SP, sp),
                             (UC_X86_REG_BX, bx), (UC_X86_REG_CS, LOAD_SEG + 0x2294), (UC_X86_REG_IP, 0x0D03)):
                    mu.reg_write(r, v)
                mu.emu_start(a.lin(LOAD_SEG + 0x2294, 0x0D03), a.lin(STUB_SEG, 0), count=10_000_000)
                assert mu.reg_read(UC_X86_REG_CS) == STUB_SEG, 'did not return'
                ax = mu.reg_read(UC_X86_REG_AX)
                res = f'{ax:x} {fnv(bytes(mu.mem_read(a.lin(DST, 0), 0x400))):x}'
            else:
                off = {'setup': 0x0084, 'shift': 0x0265, 'done': 0x04DC, 'leave': 0x027A}[op]
                if op == 'leave':
                    # A city cleared with a depth of 0 runs 65536 rows:
                    # long, not endless; a larger cap than call_far's.
                    call_far_long(a, 0x217B, off)
                else:
                    a.call_far(0x217B, off)
                after = a.seg_bytes(a.ds)
                parts = [fnv(changes(before, after)), fnv(a.seg_bytes(a.r16(0x9030))),
                         fnv(a.seg_bytes(WALLS)), fnv(b''.join(log))]
                res = ' '.join(f'{p:x}' for p in parts)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'wildrun | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
