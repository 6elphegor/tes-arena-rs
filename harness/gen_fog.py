"""Emit test vectors for weather and fog (src/weather.rs): the area's
weather on entering it (0000:0F05, `weather_set`), the sky hook's fog step
(2294:0B01, `fog_step`, with 0BEF, 0C36 and the normal table's reset 0B8A
running) and the fog's start (2294:0BEF, `fog_start`).

Usage: gen_fog.py ACD_unpacked.EXE OUT

Stubbed, counting their calls in DGROUP: 0000:6C32 (FFE0), AAA2 (FFE4;
it also sets the light table's segment CCC2 to TSEG, so the fill of its
last rows is seen), 9B5B (FFE6, near; 9B52 is `call 9B5B; retf`), 9A6E
(FFE8), 742C (FFEA, near; 7428 calls it), 7534 (FFEC, near), 9B56 (FFEE,
near). The RNG (0000:6EA8) runs.

A case fills from a seed (`setup`, mirrored in tests/fog.rs) the flags
C398 (bit 4 mostly), C39A, 5C8, fog (F81, F82, CB5C, each zero half the
time), the time of day, the fill colours, the row limits, the player's
light and the near count, the terrain (A50), zone (A51) and the zones'
weathers (0..9), A4E, the day (CCA8), the RNG's seed and 837A.

  fog | kind | seed | result

kind 0 weather_set, 1 fog_step, 2 fog_start. Results: "fault" / "hang",
or the FNV-1a digest of the changed DGROUP bytes and the table's rows
(TSEG:0D00..0F00), then for fog_step the carry flag.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from unicorn.x86_const import UC_X86_REG_EFLAGS
from gen_attack import fnv, stream
from gen_frame import counter

TSEG = 0x6000


# The inverse of the LCG's multiplier 6D002Dh mod 2**32.
INV = 0x3F154FA5


def setup(seed):
    """Writes. Mirrored in tests/fog.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    w.append((0xC398, pk(word() & 0xFFFB | (4 if byte() < 0xE0 else 0))))
    w.append((0xC39A, pk(word())))
    w.append((0x05C8, pk(word())))
    for at in (0x0F81, 0x0F82, 0xCB5C):
        w.append((at, pb(0 if byte() < 0x80 else byte())))
    k = byte() % 5
    t = (byte() % 0x100, 0x300 + byte(), 0x100 + word() % 0x201, word() & 0x3FF, (0xFF, 0x100, 0x300, 0x301)[byte() % 4])[k]
    w.append((0xCCFE, pk(t)))
    for at in (0xC3C6, 0xC3C7):
        w.append((at, pb(byte())))
    for at in (0xC8E7, 0xC8E9, 0xC8EB, 0xC3E2, 0xC3DC):
        w.append((at, pk(word())))
    w.append((0x0A50, pb(byte() % 8)))
    w.append((0x0A51, pb(byte() % 36)))
    for z in range(36):
        w.append((0x0A5A + z, pb(byte() % 10)))
    w.append((0x0A4E, pk(word())))
    w.append((0xCCA8, pk(word() & 0x1F)))
    w.append((0x91F6, pk(word())))
    w.append((0x91F8, pk(word())))
    w.append((0x837A, pk(word())))
    # Aimed cases, written over the rest: the roll's edges, the game's
    # seed chosen through the LCG's inverse (its multiplier 6D002Dh is
    # odd). In zone weather 7 the first roll fog under 3E80h (3E7Fh..3E81h);
    # in 5 (rain, 81h) the second roll thunder up to 5DC0h (5DBFh..5DC1h).
    k = byte() % 4
    if k in (1, 2):
        w.append((0x0A51, pb(0)))
        w.append((0x0A5A, pb(7 if k == 1 else 5)))
        r = (0x3E7F if k == 1 else 0x5DBF) + byte() % 3
        seed = (r << 16 | word()) * INV & 0xFFFFFFFF
        if k == 2:
            seed = seed * INV & 0xFFFFFFFF
        w.append((0x91F6, struct.pack('<I', seed)))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    aaa2 = bytes([0xC7, 0x06, 0xC2, 0xCC, TSEG & 0xFF, TSEG >> 8]) + counter(0xFFE4, True)
    for seg, off, code in [(0, 0x6C32, counter(0xFFE0, True)), (0, 0xAAA2, aaa2),
                           (0, 0x9B5B, counter(0xFFE6, False)), (0, 0x9A6E, counter(0xFFE8, True)),
                           (0, 0x742C, counter(0xFFEA, False)), (0, 0x7534, counter(0xFFEC, False)),
                           (0, 0x9B56, counter(0xFFEE, False))]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)
    rng = random.Random(0x1994_0F81)
    for n in range(int(os.environ.get('N', 4500))):
        kind = n % 3
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(TSEG, 0), zero)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            cf = ''
            if kind == 0:
                a.call_far(0, 0x0F05)
            elif kind == 1:
                a.call_near(0x2294, 0x0B01)
                cf = f' {a.mu.reg_read(UC_X86_REG_EFLAGS) & 1}'
            else:
                a.call_far(0x2294, 0x0BEF)
            after = a.seg_bytes(a.ds)
            ch = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(ch + a.seg_bytes(TSEG)[0xD00:0xF00]):x}{cf}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'fog | {kind} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
