"""Emit test vectors for a landing swing (src/strike.rs `swing_hits`)
from the original code.

Usage: gen_swing.py ACD_unpacked.EXE OUT

A case is a strike case (gen_strike.py: `setup`, then `strike`), then
from the same stream (`spread`, mirrored in tests/swing.rs) every actor
slot is placed around the player (some peaceful, corpses or out of
reach), creature slots pointing at the eight records, the player's
heading and height. It runs 0000:39D8 up to 3A93: the loop over the
actors. The wall hit when no actor was reached (0000:3D43) is stubbed
with a RET, and the stubs of gen_strike.py apply.

  swing | seed k | result   as gen_strike.py
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, DGROUP
from gen_attack import setup, fnv, CSEG, RSIZE
from gen_strike import strike, RELOC, WSEG, FRAME, ACTORS, SIZE


def spread(byte, word, w, corpse):
    """All 23 actor slots around the player at (px, py) = C3D0/C3D4 as
    `strike` set them (read back from `w`)."""
    px = py = None
    for seg, off, data in w:
        if seg is None and off == 0xC3D0:
            px = struct.unpack('<H', data)[0]
        if seg is None and off == 0xC3D4:
            py = struct.unpack('<H', data)[0]
    for i in range(23):
        si = ACTORS + i * SIZE
        if byte() < 0x20:
            continue  # left as it was
        x = (px + byte() * 2 - 0x100) & 0xFFFF
        y = (py + byte() * 2 - 0x100) & 0xFFFF
        t = byte()
        sprite = 0xC5 if t < 0x10 else corpse if t < 0x20 else 0xC0 + t % 5
        flags = word() & 0x7FFF if byte() < 0x30 else word() & 0x3FFF
        w.append((None, si, struct.pack('<HHH', x, 0, y)))
        w.append((None, si + 0xC, bytes([sprite])))
        w.append((None, si + 0xF, struct.pack('<H', flags)))
        if i >= 15:
            w.append((None, si + 0x12, struct.pack('<H', (i - 15) * RSIZE)))
    w.append((None, 0xC3DA, struct.pack('<H', word() & 0x1FF)))
    w.append((None, 0xC3D2, struct.pack('<H', 0x3C if byte() < 0xE0 else word())))
    w.append((None, 0x0FF9, struct.pack('<H', 0 if byte() < 0x80 else byte() % 8)))


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x23BA, 0x002A, b'\xcb'), (0x1194, 0xA36C, b'\xcb'), (0, 0x9AE6, b'\xcb'),
                           (0, 0x9CA0, b'\xcb'), (0x2DA2, 0x000A, b'\xcb'), (0x1194, 0x59C8, b'\xcb'),
                           (0, 0x3D43, b'\xc3')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0x1042, CSEG)
    a.w16(0xD806, WSEG)
    a.w16(0x9030, FRAME)
    a.w16(0x91F0, 0)
    classes = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
    corpse = (a.mu.mem_read(a.lin(a.ds, 0xC936), 1)[0] + a.r16(0xC38E)) & 0xFF
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)

    def unrelocated(mem):
        mem = bytearray(mem)
        for o in RELOC:
            if struct.unpack_from('<H', mem, o)[0] == a.ds:
                struct.pack_into('<H', mem, o, DGROUP)
        # The guards' hook (0000:3F21) is a far pointer to code segment 0.
        if struct.unpack_from('<H', mem, 0x67EC)[0] == LOAD_SEG:
            struct.pack_into('<H', mem, 0x67EC, 0)
        return bytes(mem)

    def extra(byte, word, w, k):
        strike(byte, word, w, k * RSIZE)
        spread(byte, word, w, corpse)

    rng = __import__('random').Random(0x1994_39D8)
    for n in range(2000):
        seed = rng.getrandbits(32)
        k = rng.randrange(8)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, WSEG, FRAME):
            a.mu.mem_write(a.lin(seg, 0), zero)
        for seg, off, data in setup(seed, k, classes, lambda byte, word, w: extra(byte, word, w, k)):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        for o in RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            a.run_range(0, 0x39D8, 0x3A93)
            after = unrelocated(a.seg_bytes(a.ds))
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'swing | {seed:x} {k} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
