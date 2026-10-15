"""Emit test vectors for the arrival's text (src/travelmap.rs
`travel_arrival`, 26F7:08F7) from the original code.

Usage: gen_arrival.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/arrival.rs) the
destination's kind (1943, a dungeon now and then), the current place's
id (A4C) and province (A46), the province shown (47E1, now and then the
Imperial one, 8), the destination's name (1929) and the province's (1429),
the clock (5CC), the days (1927), 5C8, 101C and 137E, garbage in the
text's buffer (10A8, 400h bytes) and the scratch (9C62), and the text the
stubbed copy writes: words and runs with spaces, CRs and LFs, long now
and then.

The stubs (patched to RETF; the log in the order of the calls):

  text (1194:A36C): the case's text and a NUL written at ES:DI, logged
    1, 137A
  25FD:0844: nothing
  delete (0000:9DC2): 2, SI
  write (0000:9DD2): 3, SI, CX, the low dword of the FNV-1a of the CX
    bytes at DS:DI

  arrival | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes and of the log.
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


def setup(seed):
    """(writes, text). Mirrored in tests/arrival.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x10A8, bytes(byte() | 1 for _ in range(0x400))))
    w.append((0x9C62, bytes(byte() for _ in range(0x80))))
    kind = 3 if byte() < 0x20 else byte() % 3
    w.append((0x1943, bytes([kind])))
    w.append((0x0A4C, bytes([byte()])))
    w.append((0x0A46, bytes([byte() % 9 if byte() < 0xE0 else byte()])))
    w.append((0x47E1, bytes([8 if byte() < 0x40 else byte() % 9])))
    letters = lambda n: bytes(0x61 + byte() % 26 for _ in range(n))
    w.append((0x1929, letters(byte() % 20) + b'\0'))
    w.append((0x1429, letters(byte() % 13) + b'\0'))
    w.append((0x05CC, struct.pack('<I', word() << 8 | byte())))
    w.append((0x1927, pk(word() if byte() < 0x40 else byte() % 60)))
    w.append((0x05C8, pk(word())))
    w.append((0x101C, bytes([byte()])))
    w.append((0x137E, bytes([byte()])))
    # The place's text: words of 1..12 letters, a space, a CR or an LF
    # after each; now and then a run with no space, now and then long.
    n = byte() % 40 if byte() < 0xE0 else 40 + byte() % 40
    text = bytearray()
    for _ in range(n):
        k = 1 + byte() % 12 if byte() < 0xF0 else 30 + byte() % 40
        text += letters(k)
        sep = byte()
        text += b'\r' if sep < 0x20 else b'\n' if sep < 0x30 else b' '
    return w, bytes(text)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    # DGROUP's relocated words put back to image-relative values.
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    state = {}
    reg = lambda r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
    RETF = b'\xcb'

    def stub(seg, off, fn):
        mu.mem_write(a.lin(LOAD_SEG + seg, off), RETF)
        lin = a.lin(LOAD_SEG + seg, off)
        mu.hook_add(UC_HOOK_CODE, lambda mu, addr, size, _: fn(), None, lin, lin)

    def log(*v):
        state['log'].extend(v)

    def text():
        log(1, a.r16(0x137A))
        mu.mem_write(a.lin(reg('es'), reg('di')), state['text'] + b'\0')

    def write():
        n = reg('cx')
        data = bytes(mu.mem_read(a.lin(reg('ds'), reg('di')), n)) if n else b''
        log(3, reg('si'), n, fnv(data) & 0xFFFFFFFF)

    stub(0x1194, 0xA36C, text)
    mu.mem_write(a.lin(LOAD_SEG + 0x25FD, 0x0844), RETF)
    stub(0, 0x9DC2, lambda: log(2, reg('si')))
    stub(0, 0x9DD2, write)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_08F7)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, txt = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        state.update(log=[], text=txt)
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0x26F7, 0x08F7)
            after = a.seg_bytes(a.ds)
            digests = [fnv(changes(before, after)),
                       fnv(b''.join(struct.pack('<I', v & 0xFFFFFFFF) for v in state['log']))]
            res = ' '.join(f'{d:x}' for d in digests)
        except (Fault, AssertionError, UcError):
            res = 'stuck'
        out.write(f'arrival | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
