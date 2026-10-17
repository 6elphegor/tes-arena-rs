"""Emit test vectors for an image drawn by 0000:5828 with its packed
forms (src/imgpack.rs `image_unpack`, `rle_unpack`; src/render/sky.rs
`image`, `runs`; through `render::image_page`).

Usage: gen_imgpack.py ACD_unpacked.EXE GAME_DIR OUT [N]

The heap is set up (A000h..F000h). A case (seed = its number, `setup`,
mirrored in tests/imgpack.rs) makes an image at IMG:0 (a 12-byte header:
x, y, width, height, flags, the packed forms' word) and far-calls
0000:5828 with DS:SI on it, the screen [A900] at SCREEN (filled with a
byte), the row table at A6A0 rows of 140h, the view's clip (A914..A91A)
the whole screen or a part, 9207 at times set. Flags: 0, 1 (runs into a
block), 2 (runs to the screen), 4 (LZSS from +0Ah), 8 (LZHUF: the length
at +0Ah, the stream at +0Ch), at times two of them, at times 800h (0s
left out). The runs are mostly well made, at times random.

  imgpack | seed | result

result: "fault", or FNV-1a digests of the screen's segment, of the heap
(A000h..F000h) and of the changed DGROUP bytes.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from dos import DosArena
from emu import Fault, STUB_SEG, changes, fnv, mine
from gen_attack import stream

IMG = 0x8000
SCREEN = 0x5C00


def runs(byte, total):
    """Runs covering `total` bytes exactly."""
    out = bytearray()
    while total > 0:
        n = min(total, byte() % 0x80 + 1)
        total -= n
        if byte() & 1:
            out += bytes([0x80 | (n - 1), byte() if byte() < 0xC0 else 0])
        else:
            out += bytes([n - 1]) + bytes(byte() if byte() < 0xC0 else 0 for _ in range(n))
    return bytes(out)


def setup(seed):
    """The header, the bytes after it, the clip, 9207 and the screen's
    fill. Mirrored in tests/imgpack.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = byte() % 64 + 1
    h = byte() % 40 + 1
    x = (word() % 360 - 30) & 0xFFFF
    y = (word() % 240 - 30) & 0xFFFF
    k = byte() % 12
    flags = [0, 1, 2, 4, 1, 2, 4, 3, 6, 5, 0, 4][k]
    if byte() & 1:
        flags |= 0x800
    garbage = byte() < 0x10
    if flags & 1:
        # Always well made: runs past the block rewrite the heap header
        # after it, whose links the free then follows to absolute
        # addresses (where the program is loaded decides what they hit).
        body = runs(byte, w * h)
        extra = 0
    elif flags & 2:
        body = b''.join(runs(byte, w) for _ in range(h)) if not garbage else bytes(byte() for _ in range(w * h))
        extra = 0
    elif flags & 8:
        n = byte() % 300 + 2
        body = bytes([(w * h) & 0xFF, (w * h) >> 8]) + bytes(byte() for _ in range(n))
        extra = n + 2
    elif flags & 4:
        # At most 18 bytes out for each 2 in (and a flag byte for 8):
        # an image's block is never overrun, which would rewrite the
        # ring's heap header after it with what the links then point at.
        n = byte() % (w * h // 9 + 2)
        body = bytes(byte() for _ in range(n))
        extra = n
    else:
        body = bytes(byte() for _ in range(w * h))
        extra = 0
    if byte() < 0xA0:
        clip = (0, 0, 0x13F, 0xC7)
    else:
        clip = (byte() % 100, byte() % 60, 0x13F - byte() % 100, 0xC7 - byte() % 60)
    whole = byte() < 0x20
    fill = byte()
    header = bytes(b for v in (x, y, w, h, flags, extra) for b in (v & 0xFF, v >> 8))
    return header + body, clip, whole, fill


def main():
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    total = int(sys.argv[4]) if len(sys.argv) > 4 else 2000
    a = DosArena(exe, game)
    a.init_heap()
    a.w16(0xA900, SCREEN)
    for y in range(200):
        a.w16(0xA6A0 + 2 * y, y * 0x140)
    base = a.snapshot()
    with open(out, 'w') as f:
        for n in range(total):
            img, clip, whole, fill = setup(n)
            if not mine(n, total):
                continue
            a.restore(base)
            a.mu.mem_write(a.lin(IMG, 0), img)
            a.mu.mem_write(a.lin(SCREEN, 0), bytes([fill]) * 0x10000)
            for i, v in enumerate(clip):
                a.w16(0xA914 + 2 * i, v)
            a.mu.mem_write(a.lin(a.ds, 0x9207), bytes([0xFF if whole else 0]))
            ds0 = a.seg_bytes(a.ds)
            try:
                a.call_far(0, 0x5828, ds=IMG, esi=0)
                heap = bytes(a.mu.mem_read(a.lin(0xA000, 0), 0x50000))
                ds1 = a.seg_bytes(a.ds)
                r = '%x %x %x' % (fnv(a.seg_bytes(SCREEN)), fnv(heap), fnv(changes(ds0, ds1)))
            except (Fault, UcError, AssertionError):
                r = 'fault'
            f.write(f'imgpack | {n:x} | {r}\n')


if __name__ == '__main__':
    main()
