"""Emit test vectors for src/info.rs (0000:EC03, the level .INF loader)
from the original code.

Usage: gen_info.py ACD_unpacked.EXE GAME_DIR OUT [SKIP]

Runs under the DOS file layer (harness/dos.py) with EMS on: 32 banks of
64 KB swapped through the frame at 8800h, which is also the buffer
segment. Each case loads a level (0000:E87C, ported and tested in
tests/level.rs) and then runs the INF loader on the INF its INFO chunk
names, or on another INF written over that name: every level file whose
first level has an INFO chunk, then random pairs of those and INFs. The
INF is read to a 16000-byte heap block (BA54), allocated once; the flats
go to EMS banks 8..12 (the image's C8F6/C8F7).

Lines (fields separated by " | "):
  info | mif override flags section shades fill base sides caps inf | digests
  stairs | mif x y heading speed | digests  after loading the level and
         its INF (settings as for info, all 0 but flags 0x400 and section
         0x80): the
         movement step 0000:F8DD from (x, y), next to a stair square,
         facing it; the view update (0000:DA8E) and the mouse wait
         (8FC8) stubbed. The level file's name (C37F) is the level's.
         Digests as for info, then the x and y after, and FNV-1a of the
         floor, wall and ceiling planes (32 KB each). "hang" where the
         budget runs out: with flag 0x400 the directory stays cached in
         EMS bank 0x1F, which the level reload decompresses its planes
         into, so later lookups search garbage.
  ceiling | mif inf flags | digests  as info (other settings as for
         stairs), with a ceiling: no level with an INFO chunk has one, so
         a ceiling plane at 8000h, word i = (i * 9E37h) ^ (i >> 5), and map
         flag 4 are set after the level load.
where `shades` is 0 or the segment (7000h) of 4 KB of shading bytes
(i * 7 + (i >> 8), low byte), `sides` and `caps` the 16 box side and cap
bytes (hex), and `inf` the INF used ("-": the level's own). Digests are
FNV-1a of the changed DGROUP bytes (offset, value) after the INF load
(the level load already done; the open name's segment at A90A made
image-relative before and after), of the 32 banks as the program sees
them, and of the INF buffer block. "fault" for a CPU fault; cases whose
level load fails, or whose level has no INFO chunk, are left out.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG

EXE, GAME, OUT = sys.argv[1:4]
SKIP = int(sys.argv[4]) if len(sys.argv) > 4 else 0
rng = random.Random(0x1994_EC03)
out = open(OUT, 'a' if SKIP else 'w', buffering=1)  # line-buffered: progress is visible
done = 0

FRAME, SHADES, NAME, INFO = 0x8800, 0x7000, 0x0A36, 0xE2BE
BLOCK = 0x3E80


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


a = DosArena(EXE, GAME)
a.std_devices = True  # a missing flat is read from handle 2 (see src/info.rs)
a.patch(0, 0xDA8E, b'\xc3')  # the view update (stairs)
a.patch(0, 0x8FC8, b'\xc3')  # the mouse (stairs wait for its buttons)
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
a.enable_ems(FRAME)
r = a.call_near(0, 0x51EC, eax=BLOCK)
a.w16(0xBA54, r['ax'])
a.w16(0xBA56, r['dx'])
a.w16(0xCB8A, FRAME)
a.mu.mem_write(SHADES * 16, bytes((i * 7 + (i >> 8)) & 0xFF for i in range(0x1000)))
for hook in (0xCD2E, 0xCD4A, 0xCD36, 0xA904, 0x8F70):
    assert a.r16(hook + 2) == 0, hex(hook)
BASE = a.snapshot()

bsa = open(os.path.join(GAME, a.names['GLOBAL.BSA']), 'rb').read()
count = struct.unpack_from('<H', bsa, 0)[0]
d = len(bsa) - 18 * count
NAMES = [bsa[d + 18 * i: d + 18 * i + 14].split(b'\0')[0].upper() for i in range(count)]
MIFS = sorted({n for n in NAMES if n.endswith(b'.MIF')}
              | {n.upper().encode() for n in os.listdir(GAME) if n.upper().endswith('.MIF')})
INFS = sorted(n for n in NAMES if n.endswith(b'.INF'))


def has_info(name):
    """Whether the level file's first level has an INFO chunk."""
    path = a.names.get(name.decode())
    if path is not None:
        data = open(path, 'rb').read()
    else:
        i = NAMES.index(name)
        o = 2 + sum(struct.unpack_from('<I', bsa, d + 18 * j + 14)[0] for j in range(i))
        data = bsa[o:o + struct.unpack_from('<I', bsa, d + 18 * i + 14)[0]]
    p = 6 + struct.unpack_from('<H', data, 4)[0] + 6
    while p + 6 <= len(data) and data[p:p + 4] != b'LEVL':
        if data[p:p + 4] == b'INFO':
            return True
        p += 6 + struct.unpack_from('<H', data, p + 4)[0]
    return False


INFO_MIFS = [n for n in MIFS if has_info(n)]


def unrelocate(b):
    """DGROUP bytes with the open name's segment (A90A) image-relative."""
    b = bytearray(b)
    if struct.unpack_from('<H', b, 0xA90A)[0] == a.ds:
        struct.pack_into('<H', b, 0xA90A, a.ds - LOAD_SEG)
    return b


def digests(before):
    before, after = unrelocate(before), unrelocate(a.seg_bytes(a.ds))
    changed = bytearray()
    for i in range(0x10000):
        if after[i] != before[i]:
            changed += struct.pack('<HB', i, after[i])
    banks = b''.join(a.ems_bank(b) for b in range(0x20))
    seg, off = a.r16(0xBA56), a.r16(0xBA54)
    block = bytes(a.mu.mem_read(a.lin(seg, off), BLOCK))
    return f'{fnv(changed):x} {fnv(banks):x} {fnv(block):x}'


cases = [(n, None) for n in INFO_MIFS]
cases += [(rng.choice(INFO_MIFS), rng.choice(INFS)) for _ in range(200)]
for mif, inf in cases:
    a.restore(BASE)
    override = rng.choice([0xFF] * 6 + [0])  # (a level past the last spins until the budget runs out)
    # Mostly with flag 0x400: flats the map doesn't use are then skipped,
    # as the game needs (60 flats the INFs name are not in the data, and
    # without it the original stops at the missing file).
    flags = rng.choice([0x400, 0x400, 0x600, 0x400 | rng.getrandbits(16), 0, 0x200])
    section = rng.choice([0, 0x80, 0x01, 0x81, rng.getrandbits(8)])
    shades = rng.choice([0, SHADES])
    fill = rng.getrandbits(8)
    base = rng.choice([0, 0, rng.getrandbits(5)])
    sides = bytes(rng.getrandbits(6) for _ in range(16))
    caps = bytes(rng.getrandbits(6) for _ in range(16))
    done += 1
    if done <= SKIP:
        continue
    print(done, mif.decode(), inf and inf.decode(), flush=True)
    a.mu.mem_write(a.lin(a.ds, NAME), mif.lower() + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xBB0B), bytes([override]))
    a.w16(0xC39A, flags)
    a.w16(0xCB84, 0)
    a.w16(0xCB88, 0x8000)
    a.mu.mem_write(a.lin(a.ds, 0xC8EF), bytes([section]))
    a.w16(0xCCC2, shades)
    a.mu.mem_write(a.lin(a.ds, 0xC3C6), bytes([fill]))
    a.w16(0xC38E, base)
    a.mu.mem_write(a.lin(a.ds, 0xCB49), sides)
    a.mu.mem_write(a.lin(a.ds, 0xCB39), caps)
    try:
        a._setup([0, STUB_SEG], {'si': NAME})
        a._run(0, 0xE87C, STUB_SEG, 0)
    except (Fault, AssertionError):
        continue
    if a.r16(0xCB84) != INFO:
        continue
    if inf is not None:
        a.mu.mem_write(a.lin(a.ds, INFO), inf.lower() + b'\0')
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0, 0xEC03, si=INFO)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    name = '-' if inf is None else inf.lower().decode()
    out.write(f'info | {mif.lower().decode()} {override:x} {flags:x} {section:x} {shades:x} {fill:x} {base:x} '
              f'{sides.hex()} {caps.hex()} {name} | {res}\n')


def load(mif, section):
    a.restore(BASE)
    a.mu.mem_write(a.lin(a.ds, NAME), mif.lower() + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xBB0B), b'\xff')
    a.w16(0xC39A, 0x400)
    a.w16(0xCB84, 0)
    a.w16(0xCB88, 0x8000)
    a.mu.mem_write(a.lin(a.ds, 0xC8EF), bytes([section]))
    a._setup([0, STUB_SEG], {'si': NAME})
    a._run(0, 0xE87C, STUB_SEG, 0)
    if a.r16(0xCB84) != INFO:
        return False
    a.call_near(0, 0xEC03, si=INFO)
    return True


AHEAD = [0x100, 2, -0x100, -2]
stairs = 0
for mif in INFO_MIFS:
    if stairs >= 80:
        break
    done += 1
    if done <= SKIP:
        stairs += 1
        continue
    print(done, 'stairs', mif.decode(), flush=True)
    try:
        if not load(mif, 0x80):
            continue
    except (Fault, AssertionError):
        continue
    up, down = a.mu.mem_read(a.lin(a.ds, 0xC932), 2)
    walls = a.seg_bytes(a.r16(0xD806))
    width, depth = a.r16(0xC357), a.r16(0xC359)
    spots = []
    for y in range(1, depth - 1):
        for x in range(1, width - 1):
            w = struct.unpack_from('<H', walls, y * 0x100 + x * 2)[0]
            if w & 0xFF in (up, down) and w >> 8 == w & 0xFF:
                for q in range(4):
                    n = y * 0x100 + x * 2 - AHEAD[q]
                    if struct.unpack_from('<H', walls, n)[0] == 0:
                        spots.append((n, q))
    if not spots:
        continue
    n, q = rng.choice(spots)
    sx, sy = (n & 0xFF) // 2, n >> 8
    x, y = sx * 128 + rng.randint(40, 88), sy * 128 + rng.randint(40, 88)
    heading = (q * 0x80 + rng.randint(-0x30, 0x30)) & 0x1FF
    speed = rng.choice([0x40, 0x60, 0x80])
    a.w16(0xC3D0, x)
    a.w16(0xC3D4, y)
    a.w16(0xC3D2, 0x3C)
    a.w16(0xC3DA, heading)
    a.w16(0xCB90, n)
    a.mu.mem_write(a.lin(a.ds, 0xE3A1), b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xC37F), mif.lower() + b'\0')
    before = a.seg_bytes(a.ds)
    try:
        a._setup([0, STUB_SEG], {'eax': speed, 'ebx': heading})
        a._run(0, 0xF8DD, STUB_SEG, 0)  # a level change takes more than call_far's budget
        res = digests(before) + f' {a.r16(0xC3D0):x} {a.r16(0xC3D4):x} ' + ' '.join(
            f'{fnv(a.seg_bytes(a.r16(v))[:0x8000]) if a.r16(v) else 0:x}' for v in (0xCCD0, 0xD806, 0xD748))
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    stairs += 1
    out.write(f'stairs | {mif.lower().decode()} {x:x} {y:x} {heading:x} {speed:x} | {res}\n')

CEILING = 0x8000
plane = b''.join(struct.pack('<H', ((i * 0x9E37) ^ (i >> 5)) & 0xFFFF) for i in range(0x4000))
for _ in range(60):
    mif, inf = rng.choice(INFO_MIFS), rng.choice([None, rng.choice(INFS)])
    flags = rng.choice([0x400, (rng.getrandbits(16) | 0x400) & ~0x200])
    done += 1
    if done <= SKIP:
        continue
    print(done, 'ceiling', mif.decode(), flush=True)
    a.restore(BASE)
    a.mu.mem_write(a.lin(a.ds, NAME), mif.lower() + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xBB0B), b'\xff')
    a.w16(0xC39A, flags)
    a.w16(0xCB84, 0)
    a.w16(0xCB88, 0x8000)
    a.mu.mem_write(a.lin(a.ds, 0xC8EF), b'\x80')
    try:
        a._setup([0, STUB_SEG], {'si': NAME})
        a._run(0, 0xE87C, STUB_SEG, 0)
    except (Fault, AssertionError):
        continue
    if a.r16(0xCB84) != INFO:
        continue
    a.mu.mem_write(CEILING * 16, plane)
    a.w16(0xD748, CEILING)
    a.w16(0xC398, a.r16(0xC398) | 4)
    if inf is not None:
        a.mu.mem_write(a.lin(a.ds, INFO), inf.lower() + b'\0')
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0, 0xEC03, si=INFO)
        res = digests(before)
    except Fault:
        res = 'fault'
    name = '-' if inf is None else inf.lower().decode()
    out.write(f'ceiling | {mif.lower().decode()} {name} {flags:x} | {res}\n')
out.close()
