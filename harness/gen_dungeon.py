"""Emit test vectors for dungeon generation (src/dungeon.rs).

Each line: name | writes | regs | result | memory diff | map | chunks
  writes: off:size:value (DGROUP, applied before the call); off:2:s:seg is
          a segment value (relocated for the emulator, as-is for the port)
  result: "fault", "hang", "ok", or returned registers
  diff:   linear:new for every changed byte from 3276:0000 to the end of
          DGROUP (image-relative)
  map:    for dungeon_level, offset:byte for every byte of the map segment
          (5000) that is not 0x01 below 8000 or 0x00 from 8000 (a cleared
          level is all 0x01); empty for the others, which leave it alone
  chunks: for dungeon_level, "offset=block" per chunk in load order

Stubbed (patched before anything runs, since unicorn caches translated
code): file I/O 0000:6B26 (open, returns handle 7), 6D56 (seek), 6766
(close), EB88 (seek to a random1.mif level; a hook records BP and AX
first), the MIF reader 0000:2A73, 0000:2CC6 (after-level work), the
blank.mif load 0000:E880, the province load 1194:2E18, the climate read
1194:3D91 (returns a byte the case sets), and the save writes in
26F7:0C0A (25FD:0844, 0000:67C4, 6D32, 6DDC, 6742).
"""
import random, sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, LOAD_SEG, STUB_SEG
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *

a = Arena(sys.argv[1])
rng = random.Random(1994_12)
out = open(sys.argv[2], 'w')
LO, HI = 0x3276 * 16, 0x3BA0 * 16 + 0x10000  # image-relative
MAPSEG = 0x5000
MAP_LIN = (LOAD_SEG + MAPSEG) * 16
FLOORSEG = 0x6000
FLOOR_LIN = (LOAD_SEG + FLOORSEG) * 16
HANDLE = 7


def patch(seg, off, code):
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)


RET, RETF = b'\xc3', b'\xcb'
patch(0, 0x6B26, b'\xb8' + struct.pack('<H', HANDLE) + RET)  # mov ax, 7; ret
for off in (0x6D56, 0x6766, 0xEB88, 0x2A73, 0x2CC6, 0xE880):
    patch(0, off, RET)
patch(0x1194, 0x2E18, RET)
CLIMATE_BYTE = 0x3D96
patch(0x1194, 0x3D91, b'\x2e\xa0' + struct.pack('<H', CLIMATE_BYTE) + RET)  # mov al, cs:[3D96]; ret
for seg, off in ((0x25FD, 0x0844), (0, 0x67C4), (0, 0x6D32), (0, 0x6DDC), (0, 0x6742)):
    patch(seg, off, RETF)

chunks = []


def on_block(mu, addr, size, _):
    chunks.append((mu.reg_read(UC_X86_REG_BP), mu.reg_read(UC_X86_REG_AX)))


EB88 = a.lin(LOAD_SEG, 0xEB88)
a.mu.hook_add(UC_HOOK_CODE, on_block, begin=EB88, end=EB88)


def div_faults(cs, ip):
    """Does the DIV/IDIV at CS:IP fault? Unicorn can stop on a faulting
    DIV silently, and a hang that runs out of instructions can also stop
    on a good one."""
    mu = a.mu
    op, modrm = mu.mem_read(a.lin(cs, ip), 2)
    if op not in (0xF6, 0xF7) or (modrm >> 3) & 7 not in (6, 7):
        return False
    mod, rm = modrm >> 6, modrm & 7
    if mod != 3:
        return True  # memory divisors don't occur in these routines
    regs = [UC_X86_REG_AX, UC_X86_REG_CX, UC_X86_REG_DX, UC_X86_REG_BX,
            UC_X86_REG_SP, UC_X86_REG_BP, UC_X86_REG_SI, UC_X86_REG_DI]
    v = mu.reg_read(regs[rm]) & (0xff if op == 0xF6 else 0xffff)
    if v == 0:
        return True
    if (modrm >> 3) & 7 == 7:
        return False
    ax, dx = mu.reg_read(UC_X86_REG_AX), mu.reg_read(UC_X86_REG_DX)
    if op == 0xF6:
        return ax // v > 0xff
    return ((dx << 16) | ax) // v > 0xffff


def _stop(mu, addr, size, _):
    mu.emu_stop()


def run(seg, off, far=False, stop=None, **regs):
    """Call seg:off, or run it until seg:stop. Returns 'ret', 'fault' or
    'hang'."""
    mu = a.mu
    if far:
        a._setup([0, STUB_SEG], regs)
        until = a.lin(STUB_SEG, 0)
    else:
        a._setup([0xFFF0], regs)
        until = a.lin(LOAD_SEG + seg, 0xFFF0)
    mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
    mu.reg_write(UC_X86_REG_IP, off)
    h = None
    if stop is not None:
        until = a.lin(LOAD_SEG + seg, stop)
        h = mu.hook_add(UC_HOOK_CODE, _stop, begin=until, end=until)
    a.fault = None
    mu.emu_start(a.lin(LOAD_SEG + seg, off), until, count=3_000_000)
    if h is not None:
        mu.hook_del(h)
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if a.fault is not None:
        return 'fault'
    if a.lin(cs, ip) == until:
        return 'ret'
    return 'fault' if div_faults(cs, ip) else 'hang'


base = None


def map_marks():
    m = bytes(a.mu.mem_read(MAP_LIN, 0x10000))
    return ' '.join(f'{i:x}:{b:x}' for i, b in enumerate(m) if b != (1 if i < 0x8000 else 0))


def case(name, writes, seg, off, far=False, stop=None, rets=None, climate=None, **regs):
    global base
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32))] + writes
    region = lambda: bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))
    if base is None:
        base = region()
    a.mu.mem_write(LOAD_SEG * 16 + LO, base)
    a.mu.mem_write(MAP_LIN, bytes(0x10000))
    a.mu.mem_write(FLOOR_LIN, bytes(0x10000))
    for w in writes:
        o, size, v = w[0], w[1], w[-1]
        if len(w) == 4:  # segment value
            v = (v + LOAD_SEG) & 0xffff
        a.mu.mem_write(a.lin(a.ds, o), (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    if climate is not None:
        a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, CLIMATE_BYTE), bytes([climate]))
    before = region()
    chunks.clear()
    r = run(seg, off, far, stop, **regs)
    after = region()
    if r in ('fault', 'hang'):
        result, diff, marks = r, '', ''
    else:
        if rets:
            result = ' '.join(f'{k}:{a.mu.reg_read(globals()["UC_X86_REG_" + k.upper()]):x}' for k in rets)
        else:
            result = 'ok'
        diff = ' '.join(f'{LO + i:x}:{y:x}' for i, (x, y) in enumerate(zip(before, after)) if x != y)
        marks = map_marks() if name in ('level', 'first_level') else ''
    ws = []
    for w in writes:
        o, size, v = w[0], w[1], w[-1]
        ws.append(f'{o:x}:{size}:s:{v:x}' if len(w) == 4 else f'{o:x}:{size}:{v & ((1 << 8 * size) - 1):x}')
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    if climate is not None:
        rg += f' climate:{climate:x}'
    line = f'{name} | {" ".join(ws)} | {rg} | {result} | {diff} | {marks}'
    if name in ('level', 'first_level') and r == 'ret':
        line += ' | ' + ' '.join(f'{bp:x}={blk:x}' for bp, blk in chunks)
    out.write(line + '\n')


def seed_writes():
    """A dungeon seed at 05FC; now and then a degenerate one."""
    return [(0x05FC, 4, rng.choice([rng.getrandbits(32)] * 8 + [0, 0x80000000, 0x40000000]))]


def degenerate_rng():
    return [(0x91F6, 4, rng.choice([0, 0x80000000, 0x40000000, 0x20000000]))]


def string(at, maxlen):
    n = rng.randint(0, maxlen)
    s = bytes(rng.randint(0x41, 0x7A) for _ in range(n)) + b'\0'
    return [(at + i, 1, c) for i, c in enumerate(s)]


# --- 1194:3B0C dungeon location records ---
BOUNDS = 0x143D


def bounds():
    if rng.random() < 0.85:
        vals = [rng.randint(0, 320), rng.randint(0, 200), rng.randint(1, 320), rng.randint(1, 200)]
    else:
        vals = [rng.getrandbits(16) for _ in range(4)]
    return [(BOUNDS + 2 * i, 2, v) for i, v in enumerate(vals)]


def coord(hi):
    return rng.randint(0, hi) if rng.random() < 0.85 else rng.getrandbits(16)


for _ in range(400):
    w = bounds() + string(0x1929, rng.choice([19, 19, 19, 60])) + [
        (0x193E, 2, coord(320)), (0x1940, 2, coord(200)),
        (0x47E1, 1, rng.choice([rng.randint(0, 8), rng.getrandbits(8)]))]
    case('build_location', w, 0x1194, 0x3B0C, far=True, climate=rng.getrandbits(8), ax=rng.getrandbits(8))


# --- 2294:0ED4 level count ---
def quest_slots(province, local):
    """The two quest slots, sometimes matching the location."""
    w = []
    for p, l in ((0x0FE3, 0x0FE2), (0x0FE5, 0x0FE4)):
        if rng.random() < 0.35:
            w += [(p, 1, province), (l, 1, local & 0x1F)]
        else:
            w += [(p, 1, rng.choice([province, rng.getrandbits(8)])), (l, 1, rng.getrandbits(8))]
    return w


for _ in range(300):
    prov, lid = rng.randint(0, 8), rng.getrandbits(8)
    w = [(0x0A46, 1, prov), (0x0A4C, 1, lid)] + quest_slots(prov, lid)
    case('level_count', w, 0x2294, 0x0ED4, far=True, rets=('cx',))


# --- 0000:28BC dungeon setup ---
for i in range(400):
    w = degenerate_rng() if i % 25 == 0 else []
    w += [(0xC398, 2, rng.getrandbits(16)), (0xC39A, 2, rng.getrandbits(16))]
    ax = rng.choice([0, 1, 1, 2, 3, rng.getrandbits(16), 0xFFFF])
    bx = rng.choice([0, 0, 1, 1, 2, 3, rng.getrandbits(16), 0xFFFF])
    cx = rng.choice([1, 2, 4, rng.randint(1, 30), rng.randint(1, 300)])
    case('setup', w, 0, 0x28BC, ax=ax, bx=bx, cx=cx)


# --- 0000:0DE6..0E2C entering a dungeon ---
for i in range(400):
    prov, lid = rng.randint(0, 8), rng.getrandbits(8)
    w = [(0x0A56, 4, rng.choice([rng.getrandbits(32)] * 10 + [0])), (0x05C8, 2, rng.getrandbits(16)),
         (0x0608, 2, rng.getrandbits(16)), (0x060A, 2, rng.getrandbits(16)),
         (0x0A46, 1, prov), (0x0A4C, 1, lid), (0xC398, 2, rng.getrandbits(16)), (0xC39A, 2, rng.getrandbits(16))]
    case('enter', w + quest_slots(prov, lid), 0, 0x0DE6, stop=0x0E2C)


# --- 0000:299F building a level ---
def stairs():
    return [(0x05E8 + i, 1, rng.choice([2 * rng.randint(0, 15), rng.getrandbits(8) & 0xFE, rng.getrandbits(8)]))
            for i in range(24)]


for _ in range(300):
    levels = rng.choice([1, 2, 4, rng.randint(1, 10)])
    level = rng.choice([rng.randint(0, levels - 1), rng.getrandbits(8)])
    tiles = lambda: rng.choice([0x20, 0x40, 0x40, 0x60, 0x80, rng.randint(0x20, 0xFF)])
    w = seed_writes() + stairs() + [
        (0xC357, 2, tiles()), (0xC359, 2, tiles()), (0xC355, 2, levels),
        (0xD806, 2, 's', MAPSEG), (0x6596, 2, rng.getrandbits(16))]
    case('level', w, 0, 0x299F, cx=level)


# --- 0000:2978..299E start position ---
for _ in range(200):
    case('start_position', [(0x05E8, 1, rng.choice([2 * rng.randint(0, 15), rng.getrandbits(8)]))],
         0, 0x2978, stop=0x299E)


# --- 1194:CE1B / CE40 main-quest dungeons ---
for i in range(300):
    w = degenerate_rng() if i % 10 == 0 else []
    w += [(0x0FDD, 1, rng.choice([rng.randint(0, 9), rng.randint(0, 9), rng.getrandbits(8)]))]
    case('quest_provinces', w, 0x1194, 0xCE1B)
    case('quest_target', [(0x0FDD, 1, rng.getrandbits(8)), (0x0FE6, 1, rng.getrandbits(8))], 0x1194, 0xCE40)


# --- 26F7:0C0A picking a province's dungeon entry ---
NAME_BUF = 0xF000


def entries(all_used):
    w = []
    for k in range(16):
        e = 0x1765 + 25 * k
        n = rng.choice([0, rng.randint(1, 19)])
        name = bytes(rng.randint(0x41, 0x7A) for _ in range(n)) + b'\0'
        w += [(e + i, 1, c) for i, c in enumerate(name)]
        flags = rng.getrandbits(8)
        if all_used or rng.random() < 0.5:
            flags |= 2
        else:
            flags &= ~2
        w += [(e + 0x14, 2, coord(320)), (e + 0x16, 2, coord(200)), (e + 0x18, 1, flags)]
    return w


for i in range(400):
    w = degenerate_rng() if i % 20 == 0 else []
    w += entries(rng.random() < 0.1) + string(NAME_BUF, rng.choice([0, 19, 25])) + [
        (0x1A6C, 1, rng.choice([rng.randint(0, 9), rng.getrandbits(8)]))]
    mode = rng.choice([0, 1, 1, 2, 3, 4, 4, rng.getrandbits(16)])
    case('pick_entry', w, 0x26F7, 0x0C0A, far=True, rets=('ax', 'bx', 'cx', 'si'),
         ax=rng.randint(0, 8), bx=mode, cx=rng.choice([rng.randint(0, 15), rng.getrandbits(16)]), si=NAME_BUF)


# --- 0000:2974 a made dungeon entered: its level built (2957: 299F, the
# flats' lights 081A, the INF named), the player at its start (2978) ---
for _ in range(300):
    levels = rng.choice([1, 2, 4, rng.randint(1, 10)])
    level = rng.choice([rng.randint(0, levels - 1), rng.getrandbits(8)])
    tiles = lambda: rng.choice([0x20, 0x40, 0x40, 0x60, 0x80, rng.randint(0x20, 0xFF)])
    w = seed_writes() + stairs() + [
        (0xC357, 2, tiles()), (0xC359, 2, tiles()), (0xC355, 2, levels), (0xBB0A, 1, level),
        (0xD806, 2, 's', MAPSEG), (0xCCD0, 2, 's', FLOORSEG), (0x6596, 2, rng.getrandbits(16)),
        (0x7A46, 2, rng.getrandbits(16)), (0xC765, 1, rng.getrandbits(8)), (0xC38E, 2, rng.choice([0, rng.randint(0, 0x40)]))]
    case('first_level', w, 0, 0x2974)
