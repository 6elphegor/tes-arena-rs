"""Emit test vectors for the flats' scaler (src/render.rs `blit_rows`):
0000:78A8 with its rows compiled by the routine CS:789E names, 7A2B (the
plain rows) or 9546 (a light's: 953E sets it, through the light table
by the source's byte and the screen's pixel under it).

Usage: gen_scaler.py ACD_unpacked.EXE OUT [N]

Each case starts from the same memory: the source (SSEG) and the table
(TSEG, also the colours') the LCG stream from SRCSEED and TABSEED (a
third of the source's bytes 0), the rows' offsets (A6A0) 140h apart,
the page ([A900], PSEG) from the case's seed. From the case's seed: the
routine, the window (A914..A91A), the place, the sizes, bits a pixel,
the flags, the shade's and colours' segments and offsets, the source's
offset and 78A8's [BP-14h] on the stack (9546 reads it, not setting it).
A light's 0s are always left out, as the flats' calls have it.
The case where 9546 would read the code segment through 781E's colours
(under 8 bits, colours and a shade) isn't made.

  scaler | seed | <page digest> | <rows' digest>
  scaler | seed | fault

The page's 64 KB after the call (digest: FNV-1a), and the 1000h bytes of
DGROUP from 929E, where 78A8 compiles the row it draws by (the code is
left there after). "fault": a division by 0 (a size drawn of 0).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from gen_attack import fnv, stream

SSEG = 0x7000
TSEG = 0x6000
PSEG = 0x8000
SRCSEED = 0x5CA1_E5EE
TABSEED = 0x7AB1_E5EE
SS, SP = 0x9000, 0xFFF0


def fixed(a):
    """The memory every case starts from (mirrored in tests/scaler.rs)."""
    s = stream(SRCSEED)
    src = bytearray()
    for _ in range(0x10000):
        b = next(s)
        src.append(0 if b < 0x55 else next(s))
    a.mu.mem_write(a.lin(SSEG, 0), bytes(src))
    s = stream(TABSEED)
    a.mu.mem_write(a.lin(TSEG - 0x10, 0), bytes(next(s) for _ in range(0x10100)))
    a.w16(0xA900, PSEG)
    for y in range(0xC8):
        a.w16(0xA6A0 + 2 * y, y * 0x140)


def setup(seed):
    """(light, window, args, odd) of case `seed`: the window (left, top,
    right, bottom), 78A8's 14 argument words as pushed, [BP-14h].
    Mirrored in tests/scaler.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    light = byte() & 1
    left, top = byte() % 0x60, byte() % 0x40
    right = left + byte() % (0x140 - left)
    bottom = top + byte() % (0xC8 - top)
    w = 1 + byte() % 0x60
    h = 1 + byte() % 0x60
    k = byte() % 4
    dw = (1 + byte() % 0x20) if k == 0 else (1 + byte() % 0xC0)
    dh = (1 + byte() % 0x20) if k == 1 else (1 + byte() % 0xC0)
    x = (left + byte() - 0x30) & 0xFFFF
    y = (top + byte() % 0x80 - 0x20) & 0xFFFF
    bpp = 8 if byte() < 0xA0 else 1 + byte() % 7
    flags = byte() & 3
    if light:
        # The flats' calls leave the 0s out (bit 0) always: without it a
        # shade's segment 0 reads segment 0's bytes, the harness's not the
        # image's.
        flags |= 1
    shade = (TSEG, word()) if byte() < 0xC0 else (0, 0)
    cmap = (TSEG, word()) if bpp != 8 and byte() < 0xC0 else (0, 0)
    if light and bpp != 8 and cmap[0] and shade[0]:
        shade = (0, 0)
    srcoff = word() & 0x7FFF
    odd = byte() & 1 if byte() < 0x40 else 0
    args = [x, y, dw, dh, shade[1], shade[0], w, h, bpp, flags, srcoff, SSEG, cmap[1], cmap[0]]
    return light, (left, top, right, bottom), args, odd


def page_of(seed):
    """The page's bytes before case `seed` (mirrored in tests/scaler.rs)."""
    s = stream(seed ^ 0xFA6E_0000)
    return bytes(next(s) for _ in range(0x10000))


def main():
    EXE, OUT = sys.argv[1:3]
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 3000
    a = Arena(EXE)
    fixed(a)
    ds0 = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    cs0 = bytes(a.mu.mem_read(a.lin(LOAD_SEG, 0), 0x10000))
    with open(OUT, 'w') as out:
        for i in range(n):
            seed = 0x5CA1_0000 + i
            a.mu.mem_write(a.lin(a.ds, 0), ds0)
            a.mu.mem_write(a.lin(LOAD_SEG, 0), cs0)
            a.mu.mem_write(a.lin(PSEG, 0), page_of(seed))
            light, win, args, odd = setup(seed)
            for o, v in zip((0xA914, 0xA916, 0xA918, 0xA91A), win):
                a.w16(o, v)
            a.w16(0x789E, 0x9546 if light else 0x7A2B, seg=LOAD_SEG)
            # 78A8's frame: BP is the stack's top less the return and the
            # arguments, less BP pushed; [BP-14h] the stack's byte left.
            a.mu.mem_write(a.lin(SS, 0xFF00), b'\0' * 0x100)
            a._setup([0, STUB_SEG] + args, {})
            bp = SP - 2 * (2 + len(args)) - 2
            a.mu.mem_write(a.lin(SS, bp - 0x14), bytes([odd]))
            try:
                a._run(0, 0x78A8, STUB_SEG, 0, count=10_000_000)
            except Fault:
                out.write(f'scaler | {seed:x} | fault\n')
                continue
            page = bytes(a.mu.mem_read(a.lin(PSEG, 0), 0x10000))
            rows = bytes(a.mu.mem_read(a.lin(a.ds, 0x929E), 0x1000))
            out.write(f'scaler | {seed:x} | {fnv(page):x} | {fnv(rows):x}\n')


if __name__ == '__main__':
    main()
