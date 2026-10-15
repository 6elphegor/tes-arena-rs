"""Emit test vectors for the sky's bodies (src/celestial.rs `celestial`,
0000:E41A, with the stars E6E9, the projection E51B/E032, the turns
93F0/9388/93BC, the moons' places E557, their pictures E6CB and the
clip E4D2).

Usage: gen_celestial.py ACD_unpacked.EXE OUT

Stubbed (a RET each, a code hook logging its part): 0000:742C (a file
loaded: its name), 0000:5656 (a DFA frame decoded: CX, ES, DI) and
0000:5A6A (an image drawn: DS:SI, AX, BX, CX, DX and the clip's bottom
and height A91A/A91C). The star hooks CD22 and CD26, when the case sets
them, point at 2294:03A1 and 03D7, stubbed with a RETF logging AX, BX,
CX and A91A. EMS is off, so mapping the sky's bank does nothing.

A case fills from a seed (`setup`, mirrored in tests/celestial.rs) the
weather, the time of day, the latitudes, the heading, the scale, the
centre and horizon, the view's clip, the day and the day the moons were
made for, the moons' sizes and pictures' pointers, the sun's pointer
and header, the star field (lone stars and clusters, their points) and
the star images, and the hooks. A third of the cases are aimed: the
angles 0 and the bodies ahead, so that they come on the screen.

  celestial | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes and the log of the stubs' calls.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *

FRAME = 0x6000


def setup(seed):
    """Writes (segment None: DGROUP; 'hook': a hook's dword, the
    emulator relocating its segment). Mirrored in tests/celestial.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    aimed = byte() % 3 == 0
    w.append((None, 0xBB09, bytes([0 if byte() < 0xD0 else byte()])))
    w.append((None, 0xCCFE, pk(0 if aimed else word() & 0x3FF)))
    if aimed and byte() & 1:
        w.append((None, 0xCCFE, pk(0x100 + byte())))
    w.append((None, 0xCB66, pk(0 if aimed else word())))
    w.append((None, 0xE052, pk(0 if aimed else word() & 0x1FF)))
    w.append((None, 0xC3DA, pk(0 if aimed else word() & 0x1FF)))
    w.append((None, 0xCCF4, pk(0xA0 if byte() < 0xC0 else word())))
    w.append((None, 0xCCF6, pk(0xA0 if byte() < 0xC0 else word())))
    w.append((None, 0xC3D6, pk(0xA0 if byte() < 0xC0 else word())))
    w.append((None, 0xC3D8, pk(0x49 if byte() < 0xC0 else word())))
    clip = (0, 0, 0x13F, 0x92) if byte() < 0xC0 else (word(), word(), word(), word())
    for at, v in zip((0xA914, 0xA916, 0xA918, 0xA91A), clip):
        w.append((None, at, pk(v)))
    w.append((None, 0xA91C, pk(word())))
    day = byte() % 0x20
    w.append((None, 0xCCA8, pk(day | (byte() << 8 if byte() < 0x40 else 0))))
    w.append((None, 0xCB36, bytes([day if byte() < 0x80 else byte()])))
    for at in (0xE042, 0xE044, 0xE046, 0xE048):
        w.append((None, at, pk(word())))
    for at in (0xE04A, 0xE04C, 0xE04E, 0xE050):
        w.append((None, at, pk(1 + byte() % 0x40)))
    w.append((None, 0xCCAA, pk(0x100 * (1 + byte() % 16))))
    w.append((None, 0xCCAC, pk(FRAME)))
    w.append((None, 0xCCAE, pk(0x100 * (1 + byte() % 16))))
    w.append((None, 0xCCB0, pk(FRAME)))
    sun = 0x100 * (1 + byte() % 16)
    w.append((None, 0xCD92, pk(sun)))
    w.append((None, 0xCD94, pk(FRAME)))
    w.append((FRAME, sun + 4, pk(1 + byte() % 0x40)))
    w.append((FRAME, sun + 6, pk(1 + byte() % 0x40)))
    p = 0xDA56
    for i in range(0x28):
        si = 0xD8C6 + 10 * i
        if aimed:
            xyz = ((byte() - 0x80) * 8, (byte() - 0x80) * 8, 0x300 + byte() * 0x20)
        else:
            xyz = (word(), word(), word())
        for k, v in enumerate(xyz):
            w.append((None, si + 2 * k, pk(v)))
        if byte() & 3 == 0:
            w.append((None, si + 6, pk(0)))
            w.append((None, si + 8, pk(byte() % 8 if byte() < 0xE0 else word())))
        else:
            n = 1 + byte() % 5
            w.append((None, si + 6, pk(n)))
            w.append((None, si + 8, pk(p)))
            for _ in range(n):
                w.append((None, p, pk((byte() - 0x80) // 4)))
                w.append((None, p + 2, pk((byte() - 0x80) // 4)))
                w.append((None, p + 4, pk(0x40 + byte() % 10)))
                p += 6
    for i in range(8):
        w.append((None, 0xE032 + 2 * i, pk(word())))
    w.append(('hook', 0xCD22, (0x2294, 0x03A1) if byte() < 0xC0 else (0, 0)))
    w.append(('hook', 0xCD26, (0x2294, 0x03D7) if byte() < 0xC0 else (0, 0)))
    # Aimed at the edges, written over the rest: the day's end (time
    # 2FFh..301h); a lone star straight ahead whose turned depth is 3E7h..
    # 3E9h (each turn by 0 takes one off); the first moon's place against
    # the clip's right edge, or its column at -1, 0 or 1 (the centre
    # moved; `edge`, set by main once 0000:E557 has made the place).
    edge = None
    k = byte() % 4
    if k == 1:
        w.append((None, 0xBB09, b'\0'))
        w.append((None, 0xCCFE, pk(0x2FF + byte() % 3)))
    elif k == 2:
        w.append((None, 0xBB09, b'\0'))
        for at in (0xCCFE, 0xCB66, 0xE052, 0xC3DA):
            w.append((None, at, pk(0)))
        for at, v in ((0xCCF4, 0xA0), (0xCCF6, 0xA0), (0xC3D6, 0xA0), (0xC3D8, 0x49)):
            w.append((None, at, pk(v)))
        for at, v in zip((0xA914, 0xA916, 0xA918, 0xA91A), (0, 0, 0x13F, 0x92)):
            w.append((None, at, pk(v)))
        w.append((None, 0xD8C6, pk(0)))
        w.append((None, 0xD8C8, pk(-0x20)))
        w.append((None, 0xD8CA, pk(0x3E9 + byte() % 3)))
        w.append((None, 0xD8CC, pk(0)))
        w.append((None, 0xD8CE, pk(0)))
    elif k == 3:
        w.append((None, 0xBB09, b'\0'))
        for at in (0xCCFE, 0xCB66, 0xE052, 0xC3DA):
            w.append((None, at, pk(0)))
        w.append((None, 0xCCA8, pk(4 + byte() % 4)))
        w.append((None, 0xA914, pk(0)))
        w.append((None, 0xA916, pk(0)))
        edge = (byte() & 1, byte() % 3)
    return w, edge


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    mu = a.mu
    st = {}

    def reg(r):
        return mu.reg_read(globals()['UC_X86_REG_' + r.upper()])

    def log(*ws):
        st['log'] += struct.pack(f'<{len(ws)}H', *[x & 0xFFFF for x in ws])

    def load(*_):
        name = bytes(mu.mem_read(a.lin(reg('ds'), reg('si')), 16)).split(b'\0')[0]
        log(1)
        st['log'] += name

    def decode(*_):
        log(2, reg('cx'), reg('es'), reg('di'))

    def draw(*_):
        ds = reg('ds')
        log(3, ds, reg('si'), reg('ax'), reg('bx'), reg('cx'), reg('dx'), a.r16(0xA91A), a.r16(0xA91C))

    def pixel(*_):
        log(4, reg('ax'), reg('bx'), reg('cx'), a.r16(0xA91A))

    def image(*_):
        log(5, reg('ax'), reg('bx'), reg('cx'), a.r16(0xA91A))

    for seg, off, fn, op in [(0, 0x742C, load, b'\xc3'), (0, 0x5656, decode, b'\xc3'), (0, 0x5A6A, draw, b'\xc3'),
                             (0x2294, 0x03A1, pixel, b'\xcb'), (0x2294, 0x03D7, image, b'\xcb')]:
        lin = a.lin(LOAD_SEG + seg, off)
        mu.mem_write(lin, op)
        mu.hook_add(UC_HOOK_CODE, fn, None, lin, lin)
    a.w16(0x9030, FRAME)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_E41A)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(FRAME, 0), zero)
        writes, edge = setup(seed)
        for seg, off, data in writes:
            if seg == 'hook':
                s_, o_ = data
                v = struct.pack('<HH', o_, s_ + LOAD_SEG if s_ else 0)
                a.mu.mem_write(a.lin(a.ds, off), v)
            else:
                a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        if edge is not None:
            # The first moon's place as 0000:E557 makes it, the clip's
            # right edge then put about it (nothing else of that run kept).
            saved = a.seg_bytes(a.ds)
            try:
                a.call_near(0, 0xE557)
                x = a.r16(0xE042)
            except (Fault, AssertionError):
                x = 0xFFFF
            a.mu.mem_write(a.lin(a.ds, 0), saved)
            if x != 0xFFFF:
                kind, d = edge
                if kind == 0:
                    a.w16(0xA918, (x + d - 1) & 0xFFFF)
                else:
                    a.w16(0xC3D6, (a.r16(0xC3D6) - x + d - 1) & 0xFFFF)
        st['log'] = b''
        before = a.seg_bytes(a.ds)
        try:
            a.call_near(0, 0xE41A)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(changes(before, after) + st["log"]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'celestial | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
