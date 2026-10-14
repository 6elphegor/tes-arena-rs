"""Emit test vectors for the light routines (src/light.rs): the level's
lights from its flats (`flat_lights`, 0000:0810), the lights near the
player (`lights_near`, 0000:E12E), a point's row (`light_row`,
0000:B6EB), and the moving lights packed (`moving_lights`, 0000:E106).

Usage: gen_lights.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/lights.rs): the
map's size (1..20 squares a side) and a corner of its wall and floor
planes (flats in either, some with a light in their record), the flat
records' light bytes and the first flat, the player's place, the reach,
the near list's key, the level's lights, the near lights (the player's
first) and their flags, the moving lights, the rows' limits (mostly the
game's 0Eh, 0Dh, 0Eh), the rows a light behind loses (8 or 2), and
C398 (bit 8000h, full daylight, now and then); then runs one routine
(op 0..3), op 2 at a point near the player (sometimes exactly
half the player's light's radius from it, or the whole radius, or far).

  lights | seed | op | result

Result: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes, then for op 2 the row and CF ("row cf").
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import changes
from emu import mine
from gen_attack import fnv, stream
from unicorn.x86_const import UC_X86_REG_EFLAGS

WSEG = 0x6800
FSEG = 0x7800


def setup(seed):
    """(writes, op, x, y). Mirrored in tests/lights.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    op = byte() % 4
    cols, rows = 1 + byte() % 20, 1 + byte() % 20
    w.append((None, 0xC357, pk(cols)))
    w.append((None, 0xC359, pk(rows)))
    for r in range(rows):
        for c in range(cols):
            k = byte() % 8
            sq = r << 8 | c << 1
            if k == 0:
                w.append((WSEG, sq, pk(0x8000 | byte() % 0x20)))
            elif k == 1:
                w.append((FSEG, sq, pk(byte() << 8 | 1 + byte() % 0x20)))
            elif k == 2:
                w.append((WSEG, sq, pk(word())))
    base = byte() % 0x10
    w.append((None, 0xC38E, pk(base)))
    for f in range(0x40):
        w.append((None, 0xCDA2 + 5 * f, bytes([0 if byte() < 0xA0 else 1 + byte() % 8])))
    x, y = word() % (cols * 128), word() % (rows * 128)
    w.append((None, 0xC3D0, pk(x)))
    w.append((None, 0xC3D4, pk(y)))
    w.append((None, 0xC396, pk(0x100 + word() % 0x800)))
    w.append((None, 0xC8ED, pk(((x >> 9) + (y >> 9)) if byte() < 0x40 else word())))
    n = byte() % 12
    w.append((None, 0xBC8E, pk(n)))
    for i in range(n):
        at = 0xBC90 + 6 * i
        w.append((None, at, pk(word() % (cols * 128 + 256))))
        w.append((None, at + 2, pk(word() % (rows * 128 + 256))))
        w.append((None, at + 4, pk((1 + byte() % 8) << 8)))
    m = 1 + byte() % 6
    w.append((None, 0xC3DC, pk(m)))
    for i in range(m):
        at = 0xC3DE + 6 * i
        w.append((None, at, pk(x + byte() - 0x80 if i == 0 else word() % (cols * 128))))
        w.append((None, at + 2, pk(y + byte() - 0x80 if i == 0 else word() % (rows * 128))))
        r = 0x140 if byte() < 0x80 else word() % 0xC00
        w.append((None, at + 4, pk(r)))
        if i == 0:
            player = (struct.unpack('<H', w[-3][2])[0], struct.unpack('<H', w[-2][2])[0], r)
    for i in range(16):
        w.append((None, 0xC997 + i, bytes([0 if byte() < 0xC0 else 1])))
    k = byte() % 5
    w.append((None, 0xBC8C, pk(k)))
    for i in range(k):
        at = 0xBBCC + 6 * i
        w.append((None, at, pk(x + byte() * 2 - 0x100)))
        w.append((None, at + 2, pk(y + byte() * 2 - 0x100)))
        w.append((None, at + 4, pk(word() % 0x600)))
    lim = (0xE, 0xD, 0xE) if byte() < 0x80 else (byte() % 16, byte() % 16, byte() % 16)
    for at, v in zip((0xC8E7, 0xC8E9, 0xC8EB), lim):
        w.append((None, at, pk(v)))
    w.append((None, 0xBB02, pk(8 if byte() & 1 else 2)))
    w.append((None, 0xC398, pk(word() & 0x7FFF | (0x8000 if byte() < 0x30 else 0))))
    px, py = x + byte() * 4 - 0x200, y + byte() * 4 - 0x200
    # Now and then exactly half the player's light's radius, or all of it,
    # along an axis from it, or far off.
    k = byte() % 4
    lx, ly, lr = player
    if k == 1:
        px, py = lx + (lr >> 1), ly
    elif k == 2:
        px, py = lx, ly + lr
    elif k == 3:
        px, py = x + word() % 0x1000, y + word() % 0x1000
    # Sometimes the first moving light at the point.
    if byte() < 0x60:
        w.append((None, 0xBBCC, pk(px)))
        w.append((None, 0xBBCE, pk(py)))
    # The moving lights' slots, for op 3.
    for i in range(0x20):
        at = 0xBB0C + 6 * i
        w.append((None, at, pk(0x8000 if byte() < 0x80 else word())))
        w.append((None, at + 2, pk(word())))
        w.append((None, at + 4, pk(word())))
    return w, op, px & 0xFFFF, py & 0xFFFF


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0xD806, WSEG)
    a.w16(0xCCD0, FSEG)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_B6EB)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(WSEG, 0), zero)
        a.mu.mem_write(a.lin(FSEG, 0), zero)
        writes, op, x, y = setup(seed)
        for seg, off, data in writes:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off & 0xFFFF), data)
        before = a.seg_bytes(a.ds)
        try:
            extra = ''
            if op == 0:
                a.call_far(0, 0x0810)
            elif op == 1:
                a.call_near(0, 0xE12E)
            elif op == 3:
                a.call_near(0, 0xE106)
            else:
                r = a.call_near(0, 0xB6EB, eax=x, ebx=y)
                extra = f' {r["ax"]:x} {a.mu.reg_read(UC_X86_REG_EFLAGS) & 1}'
            after = a.seg_bytes(a.ds)
            res = f'{fnv(changes(before, after)):x}' + extra
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'lights | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
