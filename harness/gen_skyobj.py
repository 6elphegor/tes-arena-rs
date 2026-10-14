"""Emit test vectors for the sky's objects (src/skyobj.rs): `sky_objects`
(2294:08B9, with the volcano 292B:029A), `sky_draw` (2294:0A5D, with
the projection 292B:057F, the volcano's drawing 292B:0201 and
lightning 2294:05A6) and `lightning` alone.

Usage: gen_skyobj.py ACD_unpacked.EXE OUT

Stubbed (a RETF each, with a code hook doing their part and logging
it): 0000:7428 (the load: the k-th call returns FRAME:100h*(k+1), or
fails where bit k of the case's mask is set; the images' headers are
filled by the setup), 0000:5828 (an image drawn), 0000:5652 and 5A66 (a
DFA frame decoded, then drawn), 0000:9A6E (a fill), 0000:78A8 (a bolt
drawn), 23BA:002A (a sound). 2294:0B01 (the fog's test) returns carry
where the byte at FFDE has bit 0. EMS is off, so mapping a bank does
nothing.

A case fills from a seed (`setup`, mirrored in tests/skyobj.rs) the
area's flags, the climate, the location's seed and place and province,
the day, the weather, the time of day, the sky's bank and the bank
table, the lists of objects and clouds, the volcano, lightning's state,
the heading, the projection's centre, horizon and scale, the view's
clip, the frame counter, the game's seed, and 16 image headers in FRAME.

  skyobj | seed | op | result

Op: 0 `sky_objects`, 1 `sky_draw`, 2 `lightning`. Results: "fault" /
"hang", or the FNV-1a digest of the changed DGROUP bytes, FRAME's first
1100h bytes and the log of the stubs' calls.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from gen_attack import fnv, stream
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *

FRAME = 0x6000


def setup(seed):
    """(writes, op, mask). Mirrored in tests/skyobj.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    op = byte() % 3
    mask = byte() if byte() < 0x60 else 0
    v = word() & ~4
    if byte() < 0xE0:
        v |= 4
    w.append((None, 0xC398, pk(v)))
    w.append((None, 0xC39A, pk(word())))
    w.append((None, 0x05C8, pk(word())))
    w.append((None, 0x0A50, pb(byte() % 7)))
    w.append((None, 0x0A56, pk(word())))
    w.append((None, 0x0A58, pk(word())))
    w.append((None, 0xCCA8, pk(byte() % 0x20)))
    k = byte() % 6
    if k == 5:
        v = byte()
    else:
        v = (0, 0, 1, 0x81, 2)[k]
    w.append((None, 0xBB09, pb(v)))
    w.append((None, 0xCCFE, pk(word() & 0x3FF)))
    w.append((None, 0x0A46, pb(3 if byte() < 0xA0 else byte() % 10)))
    if byte() < 0xA0:
        w.append((None, 0x0A48, pk(0x84 - 0xC0 + byte() + byte() // 2)))
        w.append((None, 0x0A4A, pk(0x34 - 0xC0 + byte() + byte() // 2)))
    else:
        w.append((None, 0x0A48, pk(word())))
        w.append((None, 0x0A4A, pk(word())))
    w.append((None, 0xC8F8, pb(0x14 if byte() < 0x80 else byte() % 0x24)))
    for i in range(0x20):
        w.append((None, 0x8FAA + 2 * i, pk(word())))
    n = byte() % 6
    w.append((None, 0x42B9, pb(n)))
    for i in range(n):
        w.append((None, 0x42BA + 6 * i, pk(0x100 * (1 + byte() % 16))))
        w.append((None, 0x42BC + 6 * i, pk(word())))
        w.append((None, 0x42BE + 6 * i, pk(word())))
    n = byte() % 8
    w.append((None, 0x438C, pb(n)))
    for i in range(n):
        w.append((None, 0x4396 + 6 * i, pk(0x100 * (1 + byte() % 16))))
        w.append((None, 0x4398 + 6 * i, pk(word())))
        w.append((None, 0x439A + 6 * i, pk(word())))
    k = byte() % 4
    w.append((None, 0x7BD2, pb((0, 2, 3)[k] if k < 3 else byte())))
    w.append((None, 0x7BCA, pk(word())))
    w.append((None, 0x7BCC, pk(word())))
    w.append((None, 0x7BCE, pk(0x100 * (1 + byte() % 16))))
    w.append((None, 0x7BD0, pk(FRAME)))
    k = byte() % 5
    w.append((None, 0x43C6, pk(k - 1)))
    w.append((None, 0x43C8, pk(0 if byte() < 0x20 else 0x100 * (1 + byte() % 16))))
    w.append((None, 0x43C2, pk(word())))
    w.append((None, 0x43C4, pk(word())))
    for i in range(6):
        w.append((None, 0xD8AE + 4 * i, pk(0x100 * (1 + byte() % 16))))
        w.append((None, 0xD8B0 + 4 * i, pk(FRAME)))
    w.append((None, 0xC8F3, pb(byte() % 0x20)))
    w.append((None, 0xCB90, pk(word())))
    w.append((None, 0xC3DA, pk(word() & 0x1FF)))
    w.append((None, 0xCCF4, pk(0xA0 if byte() < 0xC0 else word())))
    w.append((None, 0xC3D6, pk(0xA0 if byte() < 0xC0 else word())))
    w.append((None, 0xC3D8, pk(0x64 if byte() < 0xC0 else word())))
    for at in (0xA914, 0xA916, 0xA918, 0xA91A):
        w.append((None, at, pk(word())))
    w.append((None, 0xCCBC, pk(word())))
    w.append((None, 0x91F6, pk(word())))
    w.append((None, 0x91F8, pk(word())))
    w.append((None, 0xFFDE, pb(byte() & (1 if byte() < 0x40 else 0))))
    for i in range(16):
        h = 0x100 * (i + 1)
        w.append((FRAME, h, pk(word() if byte() < 0x40 else 1 + byte() % 8)))
        for f in (2, 8, 0xA):
            w.append((FRAME, h + f, pk(word())))
        w.append((FRAME, h + 4, pk(1 + byte() % 0x80)))
        w.append((FRAME, h + 6, pk(1 + byte() % 0x80)))
    return w, op, mask


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
        si = reg('si')
        name = bytes(mu.mem_read(a.lin(reg('ds'), si), 16)).split(b'\0')[0]
        log(1, si)
        st['log'] += name
        k = st['loads']
        st['loads'] += 1
        if k < 8 and st['mask'] >> k & 1:
            mu.reg_write(UC_X86_REG_EAX, 0)
            mu.reg_write(UC_X86_REG_EDX, 0)
        else:
            mu.reg_write(UC_X86_REG_EAX, FRAME << 16 | 0x100 * (k + 1))
            mu.reg_write(UC_X86_REG_EDX, FRAME)

    def image(*_):
        seg, si = reg('ds'), reg('si')
        log(2, seg, si, *[a.r16(si + f, seg) for f in (0, 2, 4, 6, 8)], a.r16(0xA91A))

    def dfa_decode(*_):
        st['dfa'] = (reg('ds'), reg('si'), reg('cx'))

    def dfa_draw(*_):
        seg, si, frame = st.pop('dfa')
        log(3, seg, si, frame, reg('ax'), reg('bx'))

    def fill(*_):
        log(4, reg('ax'), reg('bx'), reg('cx'), reg('dx'), mu.mem_read(a.lin(a.ds, 0x9288), 1)[0])

    def bolt(*_):
        si = a.r16(0x43C8)
        log(5, FRAME, si, a.r16(0x43C6), a.r16(si + 6, FRAME), a.r16(si + 8, FRAME))

    def sound(*_):
        log(6, reg('ax'), reg('cx'))

    for seg, off, fn in [(0, 0x7428, load), (0, 0x5828, image), (0, 0x5652, dfa_decode), (0, 0x5A66, dfa_draw),
                         (0, 0x9A6E, fill), (0, 0x78A8, bolt), (0x23BA, 0x002A, sound)]:
        lin = a.lin(LOAD_SEG + seg, off)
        mu.mem_write(lin, b'\xcb')
        mu.hook_add(UC_HOOK_CODE, fn, None, lin, lin)
    # 2294:0B01: test byte [FFDE],1; jz +2; stc; ret; clc; ret
    mu.mem_write(a.lin(LOAD_SEG + 0x2294, 0x0B01), bytes.fromhex('f606deff017402f9c3f8c3'))
    a.w16(0x9030, FRAME)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_08B9)
    for n in range(int(os.environ.get('N', 4000))):
        seed = rng.getrandbits(32)
        if os.environ.get('DEBUG') and os.environ['DEBUG'] != f'{seed:x}':
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(FRAME, 0), zero)
        writes, op, mask = setup(seed)
        for seg, off, data in writes:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        st.update(log=b'', loads=0, mask=mask)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x2294, (0x08B9, 0x0A5D, 0x05A6)[op])
            after = a.seg_bytes(a.ds)
            ch = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(ch + a.seg_bytes(FRAME)[:0x1100] + st["log"]):x}'
            if os.environ.get('DEBUG') == f'{seed:x}':
                print('ds', [(hex(i), hex(before[i]), hex(after[i])) for i in range(0x10000) if after[i] != before[i]])
                print('log', st['log'].hex())
                fr = a.seg_bytes(FRAME)[:0x1100]
                print('frame', [(hex(i), hex(fr[i])) for i in range(0x1100) if fr[i]][:400])
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'skyobj | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
