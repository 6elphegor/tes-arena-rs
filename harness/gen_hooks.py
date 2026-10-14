"""Emit test vectors for the 3D mode's hooks into segment 0000
(src/hooks.rs, and the walk's register flow in src/movement.rs).

Usage: gen_hooks.py ACD_unpacked.EXE OUT

The hooks are relocated pointers in the emulator (segment LOAD_SEG),
as the game's are; in the port their segment is 0.

Ops (a case fills from a seed, `setup_*`, mirrored in tests/hooks.rs):
  walk    0000:F8DD (a move, `move_step`) with the walk's test (CD6A:
          0000:4009) set and the 8 words at 196F drawn: 0, squares near
          the player, small words (the squares the next candidates' stale
          AX/BX make), any word; the planes as harness/gen_move.py's.
  stairs  0000:51B8 (`stairs_hook`). Stubbed, counting their calls:
          0000:6C32 (FFE0), 7428 (FFE2), AAA2 (FFE4), 292B:095E (FFE6).
  level   0000:44D7 (`level_entered`). Stubbed, counting: 292B:09AC
          (FFE0), 1194:9FAC (FFE2), 2433:0000 (FFE4), 0000:368E (FFE6).
  infdone 0000:4531 (`inf_done_hook`, 25CA:0200).

  hooks | op | seed | result

Results: "fault" / "hang", or for walk the position and an FNV-1a
digest of the changed DGROUP bytes; for the others the digest of the
changed DGROUP bytes and of the wall, floor and creature segments.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from gen_attack import fnv, stream
from gen_frame import counter

WSEG, FSEG, CSEG = 0x6800, 0x7000, 0x7800
OPS = ('walk', 'stairs', 'level', 'infdone')


def plane_words(seed):
    """As harness/gen_move.py's."""
    x, words = seed, []
    for _ in range(0x4000):
        x = (x * 1103515245 + 12345) & 0xffffffff
        r = x >> 8
        t = r % 10
        v = (r >> 4) & 0xFFFF
        if t < 3:
            words.append(0)
        elif t == 3:
            words.append((v & 0x7F) * 0x101)
        elif t == 4:
            words.append(v & 0x7FFF)
        else:
            words.append((0x8 + (t - 5) % 6) << 12 | (v & 0x0FFF))
    return words


def lcg_bytes(seed, n):
    x, b = seed, bytearray()
    for _ in range(n):
        x = (x * 1103515245 + 12345) & 0xffffffff
        b.append(x >> 16 & 0xFF)
    return bytes(b)


pk = lambda v: struct.pack('<H', v & 0xFFFF)
pb = lambda v: bytes([v & 0xFF])


def setup_walk(seed):
    """(writes, speed, angle). Planes from the seed (mirrored)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    x = 0x100 + word() % 0x7D00 if byte() < 0xE0 else word() % 0x200
    y = 0x100 + word() % 0x7D00 if byte() < 0xE0 else word() % 0x200
    w.append((None, 0xC3D0, pk(x)))
    w.append((None, 0xC3D4, pk(y)))
    k = byte() % 6
    h = byte() if k == 5 else (0x3C, 0x3C, 0x20, 0x36, 0x50)[k]
    w.append((None, 0xC3D2, pk(h)))
    w.append((None, 0xC39A, pk(word() & 0x4702)))
    w.append((None, 0xC398, pk(word() & 0x0085)))
    k = byte() % 3
    w.append((None, 0xCCCE, pk(word() if k == 2 else (0, 0x100)[k])))
    w.append((None, 0xD80C, pk(0x96 if byte() < 0x80 else word() & 0x1FF)))
    w.append((None, 0xE8C0, pk(0x30 if byte() < 0x80 else byte() & 0x7F)))
    w.append((None, 0xC38E, pk(byte() & 0xF)))
    w.append((None, 0xC3DA, pk(word() & 0x1FF)))
    w.append((None, 0xC932, pb(byte())))
    w.append((None, 0xC933, pb(byte())))
    w.append((None, 0xCB90, pk((y >> 7 << 8) + (x >> 7 << 1))))
    w.append((None, 0xC917, bytes(byte() for _ in range(16))))
    for i in range(8):
        k = byte() % 5
        if k == 0:
            v = 0
        elif k == 1:
            dx, dy = byte() % 3, byte() % 3
            v = (((y >> 7) + dy - 1) & 0xFF) << 8 | (((x >> 7) + dx - 1) * 2 & 0xFF)
        elif k == 2:
            v = (byte() % 4) << 8 | (byte() % 2) * 2
        elif k == 3:
            v = 0 if byte() < 0x80 else word()
        else:
            v = word()
        w.append((None, 0x196F + 2 * i, pk(v)))
    w.append((None, 0x8F94, pk(word())))
    w.append((None, 0x8F78, pk(byte())))
    w.append((None, 0x8F7A, pk(0xC7 if byte() < 0xC0 else word())))
    k = byte() % 5
    speed = word() & 0x1FF if k == 4 else (0x10, 0x20, 0x40, 0x80)[k]
    angle = word() & 0x1FF
    return w, speed, angle


def setup_stairs(seed):
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    for at in (0xC8E7, 0xC8E9, 0xC8EB, 0xCCC2, 0x3C8B):
        w.append((None, at, pk(word())))
    for at in (0xC3C6, 0xC3C7, 0x0F81, 0x0F82, 0x3C82):
        w.append((None, at, pb(byte())))
    return w


def flats(byte, word, w, n):
    """`n` map flats (type 8) on the wall plane, their numbers from a
    small set so the lists match some."""
    for _ in range(n):
        sq = (byte() % 0x40) << 8 | (byte() % 0x40) << 1
        w.append((WSEG, sq, pk(0x8000 | (byte() & 0x0F) << 8 | byte() % 0x20)))


def setup_level(seed):
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    w.append((None, 0x1042, pk(CSEG)))
    recs = [0x40 * (1 + i) for i in range(10)]
    for i in range(8):
        si = 0x0C57 + i * 0x1C
        w.append((None, si + 0xF, pk(word() & 0xBFFF | (0x4000 if byte() < 0x60 else 0))))
        w.append((None, si + 0x12, pk(recs[byte() % 10])))
    for r in recs:
        w.append((CSEG, r + 0x40D, pk(word())))
    for i in range(16):
        e = 0x0660 + i * 0x3C
        if byte() < 0x80:
            w.append((None, e + 0x28, pk(recs[byte() % 10])))
            w.append((None, e + 0x2A, pk(CSEG)))
        else:
            w.append((None, e + 0x28, pk(word())))
            w.append((None, e + 0x2A, pk(word())))
        w.append((None, e, bytes(byte() for _ in range(8))))
        w.append((None, e + 0x3A, pb(byte())))
    for i in range(12):
        w.append((None, 0x7DBC + i * 0x1C + 0x11, pb(byte())))
    for i in range(8):
        w.append((None, 0x7F86 + i * 0x16 + 0xA, pk(word())))
    for at in (0x7DB8, 0x0D5A, 0x7A46):
        w.append((None, at, pk(word())))
    w.append((None, 0x4C91, pb(byte())))
    k = byte() % 4
    w.append((None, 0x05E6, pb(byte() if k == 3 else (3, 3, 4)[k])))
    levels = 1 + byte() % 6
    w.append((None, 0xC355, pb(levels)))
    w.append((None, 0xBB0A, pb(levels - 1 if byte() < 0xA0 else byte() % 8)))
    place, sub = byte() % 9, byte()
    w.append((None, 0x0A46, pb(place)))
    w.append((None, 0x0A4C, pb(sub)))
    for p, l in ((0x0FE3, 0x0FE2), (0x0FE5, 0x0FE4)):
        if byte() < 0x60:
            w.append((None, p, pb(place)))
            w.append((None, l, pb(sub & 0x1F)))
        else:
            w.append((None, p, pb(byte() % 9)))
            w.append((None, l, pb(byte() & 0x1F)))
    n = 1 + byte() % 8
    w.append((None, 0xC765, pb(n)))
    for i in range(n):
        w.append((None, 0xC6E5 + 2 * i, pk((byte() % 0x40) << 8 | byte() % 0x40)))
    w.append((None, 0xC93B, pb(byte())))
    flats(byte, word, w, byte() % 24)
    for _ in range(byte() % 16):
        w.append((FSEG, (byte() % 0x40) << 8 | (byte() % 0x40) << 1, pk(word() & 0xFF1F)))
    w.append((None, 0xC943, pb(byte() % 0x20)))
    w.append((None, 0xC987, bytes(byte() % 0x20 for _ in range(8))))
    w.append((None, 0x191D, pb(0 if byte() < 0x80 else 1)))
    w.append((None, 0x0A47, pb(3 if byte() < 0xA0 else byte())))
    c941 = byte() % 0x20
    w.append((None, 0xC941, pb(c941)))
    for at, small in ((0xC357, 0), (0xC359, 1)):
        k = byte() % 4
        w.append((None, at, pk(byte() % 0x80 if k == 3 else (0x40, 0x20, small)[k])))
    f77 = byte() % 8
    w.append((None, 0x0F77, pb(f77)))
    w.append((None, 0x80EC + f77, pb(place if byte() < 0x80 else byte() % 9)))
    w.append((None, 0x0F7D, pb(0 if byte() < 0x80 else byte())))
    w.append((None, 0xC954, bytes(byte() % 0x20 for _ in range(0x40))))
    k = byte() % 4
    w.append((None, 0xC3D2, pk(word() if k == 3 else (0x3C, 0, 0xFFF0)[k])))
    sq = (1 + byte() % 0x3E) << 8 | (1 + byte() % 0x3E) << 1
    w.append((None, 0xCB90, pk(sq)))
    if byte() < 0xA0:
        v = byte() | 1
        hi = v if byte() < 0xC0 else byte()
        w.append((WSEG, sq, pk(hi << 8 | v)))
        for off in (0x100, 0xFF00, 2, 0xFFFE):
            if byte() < 0x80:
                w.append((WSEG, (sq + off) & 0xFFFF, pk(0x0101)))
    for at in (0xBA62, 0xBA66):
        w.append((None, at, pk(word())))
    # The flat 2294:119C looks for, now and then, and the rows' last words
    # (column 127), which it clears.
    if byte() < 0x90:
        w.append((WSEG, (byte() % 0x40) << 8 | (byte() % 0x40) << 1, pk(0x8000 | c941)))
    w.append((WSEG, 0x00FE, pk(0x0202)))
    for _ in range(8):
        w.append((WSEG, (byte() % 0x40) << 8 | 0xFE, pk(0x0303)))
    return w


def setup_infdone(seed):
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    flats(byte, word, w, byte() % 40)
    for _ in range(byte() % 8):
        w.append((WSEG, (byte() % 0x40) << 8 | (byte() % 0x40) << 1, pk(word())))
    w.append((None, 0xC954, bytes(byte() % 0x20 for _ in range(0x32))))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG, 0xDA8E), b'\xc3')  # the view's update: stubbed
    stubs = {
        'stairs': [(0, 0x6C32, 0xFFE0), (0, 0x7428, 0xFFE2), (0, 0xAAA2, 0xFFE4), (0x292B, 0x095E, 0xFFE6)],
        'level': [(0x292B, 0x09AC, 0xFFE0), (0x1194, 0x9FAC, 0xFFE2), (0x2433, 0x0000, 0xFFE4), (0, 0x368E, 0xFFE6)],
    }
    for lst in stubs.values():
        for seg, off, at in lst:
            a.mu.mem_write(a.lin(LOAD_SEG + seg, off), counter(at, True))
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_4009)
    n_each = int(os.environ.get('N', 1500))
    for n in range(n_each * len(OPS)):
        op = OPS[n % len(OPS)]
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (WSEG, FSEG, CSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        if op == 'walk':
            w, speed, angle = setup_walk(seed)
            a.mu.mem_write(a.lin(WSEG, 0), struct.pack('<16384H', *plane_words(seed)))
            a.mu.mem_write(a.lin(FSEG, 0), struct.pack('<16384H', *plane_words(seed ^ 0x5A5A5A5A)))
            a.mu.mem_write(a.lin(a.ds, 0xCDA1), lcg_bytes(seed ^ 0x33333333, 0x200))
            a.w16(0xCD6A, 0x4009)
            a.w16(0xCD6C, LOAD_SEG)
        else:
            w = {'stairs': setup_stairs, 'level': setup_level, 'infdone': setup_infdone}[op](seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before = a.seg_bytes(a.ds)
        try:
            if op == 'walk':
                a.call_far(0, 0xF8DD, eax=speed, ebx=angle)
            else:
                a.call_far(0, {'stairs': 0x51B8, 'level': 0x44D7, 'infdone': 0x4531}[op])
            after = a.seg_bytes(a.ds)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            if op == 'walk':
                res = f'{a.r16(0xC3D0):x} {a.r16(0xC3D4):x} {fnv(changed):x}'
            else:
                segs = ''.join(f' {fnv(a.seg_bytes(sg)):x}' for sg in (WSEG, FSEG, CSEG))
                res = f'{fnv(changed):x}{segs}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'hooks | {op} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
