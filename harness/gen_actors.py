"""Emit test vectors for src/actors.rs (0000:1854, the actors' frame)
from the original code.

Usage: gen_actors.py ACD_unpacked.EXE GAME_DIR OUT [SKIP]

Each case loads a level with an INFO chunk and its INF (as gen_info.py
does, with EMS on), puts the player on a free square, fills the actors
from a seed (`setup`, mirrored in tests/actors.rs) and runs one frame of
the actors. The door hooks are the game's: the lock check 2826:0AAB and
the sound 23BA:002A (stubbed with a RETF, as sound is not ported, like
0000:1B7F and FE58, which build the draw list), and so is the message
box of a broken special item (1194:A36C) in a creature's blow. Casting
runs for real, with gen_spell.py's stops (a projectile spawned by an
effect, 29CF:0270, is an INT 3; a spell effect leaving DS at the
creature segment or dispatching outside its handlers stops a case) and
the timed-spell-flag table at 5C00:0000.

Lines (fields separated by " | "):
  actors | mif px py seed var=value,... | digests
  door | mif px py seed cx cy lock var=value,... | digests  as actors,
       then the first creature slot is put at (cx, cy) next to a closed
       door, hostile and still, with the player on the door's far side,
       so its chase walks into the door; `lock` 1 adds a lock record for
       the door's square (row << 8 | column, at C624; count C6E4 + 1).
with the DGROUP variables of VARS (hex; sizes in VARS), and digests:
FNV-1a of the changed DGROUP bytes (offset, value), of the wall plane
(32 KB) and of the creature segment (32 KB at 7800h). "hang" where the
budget runs out.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import DGROUP, Fault, LOAD_SEG, STUB_SEG
from emu import changes
from gen_spell import guard_dispatch
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_DS

EXE, GAME, OUT = sys.argv[1:4]
SKIP = int(sys.argv[4]) if len(sys.argv) > 4 else 0
rng = random.Random(0x1994_1860)
out = open(OUT, 'a' if SKIP else 'w', buffering=1)  # line-buffered: progress is visible

FRAME, NAME, INFO, BLOCK, CSEG = 0x8800, 0x0A36, 0xE2BE, 0x3E80, 0x7800
ACTORS, COUNT, SIZE = 0x0AB3, 23, 0x1C
VARS = [(0xC3D2, 2), (0x05BE, 2), (0x653E, 2), (0x05E0, 2), (0x7A54, 2), (0xCCBA, 2), (0xAFF9, 1),
        (0xCCBC, 2), (0xC398, 2), (0xC39A, 2), (0x05C8, 2), (0x05E2, 1), (0x0F83, 1), (0x100D, 1),
        (0x1013, 1), (0x05B1, 2), (0x0604, 4), (0x392D, 1), (0x392E, 4), (0x7A44, 2), (0x91F6, 4)]


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


def stream(seed):
    x = seed
    while True:
        x = (x * 1103515245 + 12345) & 0xffffffff
        yield x >> 16 & 0xFF


def setup(seed, px, py, corpse):
    """The seeded part of a case: (segment, offset, bytes) writes, in
    order. Mirrored in tests/actors.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    raw = lambda n: bytes(byte() for _ in range(n))
    w = []
    # The creature segment, then each actor's record (at i * 0x500).
    w.append((CSEG, 0, raw(0x8000)))
    for i in range(COUNT):
        rec = i * 0x500
        a, b, c, d, e = (byte() for _ in range(5))
        f40d = (0x1000 if i >= 15 else 0) | (0x40 if a & 0xF == 0 else 0) | (0x100 if a & 0xF == 1 else 0)
        w.append((CSEG, rec + 0x40D, struct.pack('<H', f40d)))
        w.append((CSEG, rec + 0x41A, struct.pack('<H', (b & 1) | (b & 2) << 8 if b & 0xC == 0 else 0)))
        w.append((CSEG, rec + 4, bytes([c % 0x30])))
        w.append((CSEG, rec + 5, bytes([(d & 0x1F) | (0x20 if d >= 0xC0 else 0)])))
        w.append((CSEG, rec + 0x366, bytes([e & 3 if e >= 0xC0 else 0])))
    # Spell effects, some on actor records.
    w.append((None, 0x0660, raw(0x3C0)))
    for k in range(16):
        t, r = byte(), byte()
        if t < 0x40:
            w.append((None, 0x0660 + k * 0x3C + 0x28, struct.pack('<HH', (r % COUNT) * 0x500, CSEG)))
    # The effect list beside the actors, and the creature marks.
    w.append((None, 0x5F6A, raw(COUNT * SIZE)))
    w.append((None, 0x0622, raw(8 * 6)))
    for k in range(8):
        t, v = byte(), byte()
        if t < 0x40:
            w.append((None, 0x0622 + k * 6, struct.pack('<HH', ACTORS + (t % COUNT) * SIZE, v & 0x3E)))
    # The actors.
    w.append((None, ACTORS, raw(COUNT * SIZE)))
    for i in range(COUNT):
        si = ACTORS + i * SIZE
        v = byte()
        mask, half = (0x3FF, 0x200) if v < 0x80 else (0xFFF, 0x800) if v < 0xE0 else (0x1FFF, 0x1000)
        x = (px + (word() & mask) - half) & 0xFFFF
        y = (py + (word() & mask) - half) & 0xFFFF
        v = byte()
        z = 0 if v < 0xC0 else v & 3
        v = byte()
        k = v % 8
        sprite = 0xC0 + k if k < 5 else 0xC5 if k == 5 else corpse if k == 6 else v
        frame = byte() % 0xD
        flags = word() & 0x48D7
        if byte() < 0xC0:
            flags &= ~0x4001
        heading = word() & 0x1FF
        sounded = byte() & 1
        w.append((None, si, struct.pack('<HHH', x, z, y)))
        w.append((None, si + 0xA, struct.pack('<H', heading)))
        w.append((None, si + 0xC, bytes([sprite, frame])))
        w.append((None, si + 0xF, struct.pack('<H', flags)))
        w.append((None, si + 0x12, struct.pack('<H', i * 0x500)))
        w.append((None, si + 0x18, struct.pack('<H', sounded)))
    return w


a = DosArena(EXE, GAME)
a.std_devices = True
for seg, off, code in [(0, 0x1B7F, b'\xc3'), (0, 0xFE58, b'\xc3'), (0, 0xDA8E, b'\xc3'), (0, 0x8FC8, b'\xc3'),
                       (0x23BA, 0x002A, b'\xcb'), (0x1194, 0xA36C, b'\xcb')]:
    a.patch(seg, off, code)
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
a.enable_ems(FRAME)
r = a.call_near(0, 0x51EC, eax=BLOCK)
a.w16(0xBA54, r['ax'])
a.w16(0xBA56, r['dx'])
a.w16(0xCB8A, FRAME)
a.w16(0x1042, CSEG)
a.w16(0xCCCE, 0x0C00 + FRAME)  # solid floors, as entering a level sets them (0000:A49C)
a.w16(0xCD62, 0x0AAB)
a.w16(0xCD64, LOAD_SEG + 0x2826)
a.w16(0xCD46, 0x002A)
# Casting: the timed-spell-flag table, and gen_spell.py's stops.
a.w16(0x1020, 0)
a.w16(0x1022, 0x5C00)
guard_dispatch(a)


def leak(mu, addr, size, _):
    if mu.reg_read(UC_X86_REG_DS) != a.ds:
        a.fault = 'ds'
        mu.emu_stop()


for seg_, off_ in [(0x2CDD, 0x03E5), (0x2CDD, 0x003F), (0x29CF, 0x09A3)]:
    lin = (LOAD_SEG + seg_) * 16 + off_
    a.mu.hook_add(UC_HOOK_CODE, leak, None, lin, lin)
a.w16(0xCD48, LOAD_SEG + 0x23BA)
BASE = a.snapshot()

bsa = open(os.path.join(GAME, a.names['GLOBAL.BSA']), 'rb').read()
count = struct.unpack_from('<H', bsa, 0)[0]
d = len(bsa) - 18 * count
NAMES = [bsa[d + 18 * i: d + 18 * i + 14].split(b'\0')[0].upper() for i in range(count)]


def unrelocate(before, c0):
    """Words a blow or a spell sets to DGROUP's segment (the message
    pointer at 061C, effect slot targets, cast casters and the target
    list) or the spells' (cast +2, 3276h) are compared image-relative."""
    for o in [0x061C] + [0x0660 + i * 0x3C + 0x2A for i in range(16)] + \
            [0x0D5E + 4 * i for i in range(16)] + [0x7F86 + 0x16 * i + 6 for i in range(8)]:
        if a.r16(o) == a.ds and struct.unpack_from('<H', before, o)[0] != a.ds:
            a.w16(o, DGROUP)
    for o in [0x7F86 + 0x16 * i + 2 for i in range(8)]:
        if a.r16(o) == LOAD_SEG + 0x3276 and struct.unpack_from('<H', before, o)[0] != a.r16(o):
            a.w16(o, 0x3276)
    if a.r16(0x061C, CSEG) == a.ds and struct.unpack_from('<H', c0, 0x061C)[0] != a.ds:
        a.w16(0x061C, DGROUP, CSEG)


def has_info(name):
    i = NAMES.index(name)
    o = 2 + sum(struct.unpack_from('<I', bsa, d + 18 * j + 14)[0] for j in range(i))
    data = bsa[o:o + struct.unpack_from('<I', bsa, d + 18 * i + 14)[0]]
    p = 6 + struct.unpack_from('<H', data, 4)[0] + 6
    while p + 6 <= len(data) and data[p:p + 4] != b'LEVL':
        if data[p:p + 4] == b'INFO':
            return True
        p += 6 + struct.unpack_from('<H', data, p + 4)[0]
    return False


MIFS = [n for n in sorted({n for n in NAMES if n.endswith(b'.MIF')}) if has_info(n)]
LOADED = {}


def load(mif):
    """The level and its INF loaded (flag 0x400, floors section), cached."""
    if mif not in LOADED:
        a.restore(BASE)
        a.mu.mem_write(a.lin(a.ds, NAME), mif.lower() + b'\0')
        a.mu.mem_write(a.lin(a.ds, 0xBB0B), b'\xff')
        a.w16(0xC39A, 0x400)
        a.w16(0xCB84, 0)
        a.w16(0xCB88, 0x8000)
        a.mu.mem_write(a.lin(a.ds, 0xC8EF), b'\x80')
        a._setup([0, STUB_SEG], {'si': NAME})
        a._run(0, 0xE87C, STUB_SEG, 0)
        a.call_near(0, 0xEC03, si=INFO)
        LOADED[mif] = a.snapshot()
    a.restore(LOADED[mif])


def door_case(x, y, cx, cy, lock):
    """The first creature slot made a still, hostile creature at (cx, cy)
    with a fast, non-peaceful record; a lock on square (x, y) if `lock`.
    Mirrored in tests/actors.rs."""
    si = ACTORS + 15 * SIZE
    a.w16(si, cx)
    a.w16(si + 2, 0)
    a.w16(si + 4, cy)
    a.mu.mem_write(a.lin(a.ds, si + 0xC), bytes([0xC0, 0]))
    a.w16(si + 0xF, 4)
    rec = 15 * 0x500
    a.w16(rec + 0x40D, 0x1000, seg=CSEG)
    a.w16(rec + 0x41A, 0, seg=CSEG)
    a.mu.mem_write(a.lin(CSEG, rec + 4), bytes([1]))
    a.mu.mem_write(a.lin(CSEG, rec + 0x2D), bytes([0xFF]))
    a.mu.mem_write(a.lin(CSEG, rec + 0x366), bytes([0]))
    if lock:
        n = a.mu.mem_read(a.lin(a.ds, 0xC6E4), 1)[0]
        a.mu.mem_write(a.lin(a.ds, 0xC624 + 3 * n), struct.pack('<HB', y << 8 | x, 1))
        a.mu.mem_write(a.lin(a.ds, 0xC6E4), bytes([n + 1]))


done = 0
for _ in range(1500):
    mif = rng.choice(MIFS)
    seed = rng.getrandbits(32)
    pick = rng.getrandbits(32)
    vals = {0xC3D2: rng.choice([0x3C, 0x3C, 0x3C, 0x20, 0xFFF0]),
            0x05BE: rng.choice([0, 0, 1, 2, 3]),
            0x653E: rng.choice([0, 0, 1]),
            0x05E0: rng.choice([0, 0x10]),
            0x7A54: rng.choice([0x77E, 0]),
            0xCCBA: rng.choice([0, 1]),
            0xAFF9: rng.getrandbits(2),
            0xCCBC: rng.getrandbits(16),
            0x05C8: rng.choice([0, 0x1000]),
            0x05E2: rng.getrandbits(8),
            0x0F83: rng.choice([0, 0, 1]),
            0x100D: rng.getrandbits(8),
            0x1013: rng.choice([0, 0, 1]),
            0x05B1: rng.choice([0, 0x40]),
            0x0604: rng.getrandbits(32),
            0x392D: rng.choice([0, 1, 4]),
            0x392E: rng.getrandbits(32),  # (half the time a copy of 0604, below)
            0x7A44: rng.getrandbits(16),
            0x91F6: rng.getrandbits(32)}
    if rng.random() < 0.5:
        vals[0x392E] = vals[0x0604]
    flags398 = rng.choice([0, 0, 0, 0x8000])  # respawns hang on small levels
    flags39a = rng.choice([0, 0, 0x200])
    done += 1
    if done <= SKIP:
        continue
    print(done, mif.decode(), flush=True)
    try:
        load(mif)
    except (Fault, AssertionError):
        continue
    walls = a.seg_bytes(a.r16(0xD806))
    width, depth = a.r16(0xC357), a.r16(0xC359)
    free = [(x, y) for y in range(1, depth - 1) for x in range(1, width - 1)
            if struct.unpack_from('<H', walls, y * 0x100 + x * 2)[0] == 0]
    if not free:
        continue
    sx, sy = free[pick % len(free)]
    px, py = sx * 128 + 0x40, sy * 128 + 0x40
    vals[0xC398] = a.r16(0xC398) | flags398
    vals[0xC39A] = a.r16(0xC39A) | flags39a
    corpse = (a.mu.mem_read(a.lin(a.ds, 0xC936), 1)[0] + a.r16(0xC38E)) & 0xFF
    for seg, off, data in setup(seed, px, py, corpse):
        a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
    for o, n in VARS:
        a.mu.mem_write(a.lin(a.ds, o), vals[o].to_bytes(n, 'little'))
    a.w16(0xC3D0, px)
    a.w16(0xC3D4, py)
    a.w16(0xCB90, sy * 0x100 + sx * 2)
    before = a.seg_bytes(a.ds)
    c0 = a.seg_bytes(CSEG)
    try:
        a._setup([0, STUB_SEG], {})
        a._run(0, 0x1854, STUB_SEG, 0)
        unrelocate(before, c0)
        after = a.seg_bytes(a.ds)
        changed = changes(before, after)
        res = f'{fnv(changed):x} {fnv(a.seg_bytes(a.r16(0xD806))[:0x8000]):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x}'
        if os.environ.get('DUMP') == f'{seed:x}':
            for i in range(0x10000):
                if after[i] != before[i]:
                    print(f'{i:04x} {before[i]:02x} {after[i]:02x}', file=sys.stderr)
            c1 = a.seg_bytes(CSEG)
            for i in range(0x8000):
                if c1[i] != c0[i]:
                    print(f'c{i:04x} {c0[i]:02x} {c1[i]:02x}', file=sys.stderr)
            sys.exit(0)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    kv = ','.join(f'{o:x}={vals[o]:x}' for o, _ in VARS)
    out.write(f'actors | {mif.lower().decode()} {px:x} {py:x} {seed:x} {kv} | {res}\n')

# --- creatures walking into doors ---------------------------------------------

STEPS = [(1, 0), (-1, 0), (0, 1), (0, -1)]
for _ in range(300):
    mif = rng.choice(MIFS)
    seed = rng.getrandbits(32)
    pick = rng.getrandbits(32)
    lock = rng.getrandbits(1)
    vals = {0xC3D2: 0x3C, 0x05BE: 0, 0x653E: 0, 0x05E0: 0, 0x7A54: 0x77E, 0xCCBA: 0, 0xAFF9: 0,
            0xCCBC: rng.getrandbits(16), 0x05C8: 0, 0x05E2: 0, 0x0F83: 0, 0x100D: 0x80, 0x1013: 0,
            0x05B1: 0, 0x0604: 0, 0x392D: 0, 0x392E: 0, 0x7A44: 0xFFFF, 0x91F6: rng.getrandbits(32)}
    done += 1
    if done <= SKIP:
        continue
    print(done, 'door', mif.decode(), flush=True)
    try:
        load(mif)
    except (Fault, AssertionError):
        continue
    walls = a.seg_bytes(a.r16(0xD806))
    wd = lambda x, y: struct.unpack_from('<H', walls, y * 0x100 + x * 2)[0]
    width, depth = a.r16(0xC357), a.r16(0xC359)
    spots = []
    for y in range(3, depth - 3):
        for x in range(3, width - 3):
            w = wd(x, y)
            if w >> 12 == 0xB and w & 0xF00 == 0:
                for dx, dy in STEPS:
                    if wd(x - dx, y - dy) == 0 and wd(x + dx, y + dy) == 0 and wd(x + 2 * dx, y + 2 * dy) == 0:
                        spots.append((x, y, dx, dy))
    if not spots:
        continue
    x, y, dx, dy = spots[pick % len(spots)]
    px, py = (x + 2 * dx) * 128 + 0x40, (y + 2 * dy) * 128 + 0x40
    cx, cy = (x - dx) * 128 + 0x40 + dx * 0x30, (y - dy) * 128 + 0x40 + dy * 0x30
    vals[0xC398] = a.r16(0xC398)
    vals[0xC39A] = a.r16(0xC39A) | 2  # creatures open doors on levels with flag 2
    corpse = (a.mu.mem_read(a.lin(a.ds, 0xC936), 1)[0] + a.r16(0xC38E)) & 0xFF
    for seg, off, data in setup(seed, px, py, corpse):
        a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
    for o, n in VARS:
        a.mu.mem_write(a.lin(a.ds, o), vals[o].to_bytes(n, 'little'))
    a.w16(0xC3D0, px)
    a.w16(0xC3D4, py)
    a.w16(0xCB90, (py >> 7 << 8) + (px >> 7 << 1))
    door_case(x, y, cx, cy, lock)
    before = a.seg_bytes(a.ds)
    c0 = a.seg_bytes(CSEG)
    try:
        a._setup([0, STUB_SEG], {})
        a._run(0, 0x1854, STUB_SEG, 0)
        unrelocate(before, c0)
        after = a.seg_bytes(a.ds)
        changed = changes(before, after)
        res = f'{fnv(changed):x} {fnv(a.seg_bytes(a.r16(0xD806))[:0x8000]):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x}'
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    kv = ','.join(f'{o:x}={vals[o]:x}' for o, _ in VARS)
    out.write(f'door | {mif.lower().decode()} {px:x} {py:x} {seed:x} {cx:x} {cy:x} {lock} {kv} | {res}\n')
out.close()
