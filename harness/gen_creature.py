"""Emit test vectors for creature and NPC generation (src/creature.rs).

Each line: name | writes | regs | returned regs | memory diff
  writes: seg:off:size:value (image-relative segment, hex), or
          seg:off:2:s:segval for a segment value, which the emulator
          relocates (Rust sees it unrelocated)
  regs:   name:value; ds/es/fs/gs are image-relative segments
  rets:   name:value, "fault" (divide error) or "hang" (budget spent)
  diff:   linear:new for every changed byte in the compared regions,
          linear being image-relative (segment*16 + offset)

Code patches (applied once, before anything runs, since unicorn caches
translated code): 1194:080B (NPC name generation) returns at once.
"""
import random, sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, LOAD_SEG
from unicorn import UcError
from unicorn.x86_const import *

a = Arena(sys.argv[1])
rng = random.Random(1994_10)
out = open(sys.argv[2], 'w')

DG = 0x3BA0
CSEG = 0x4C00   # creature records (8 × 41E) and a far target at 3000
TSEG = 0x5000   # RNG table for the creature segment's RNG state
SSEG = 0x5100   # NPC seed records
REGIONS = [
    # The effect message text (DGROUP 1405..1427) is not ported.
    (0x3276 * 16, DG * 16 + 0x1405),
    (DG * 16 + 0x1428, DG * 16 + 0x10000),
    (CSEG * 16, CSEG * 16 + 0x3100),
    (CSEG * 16 + 0x91EE, CSEG * 16 + 0x91FC),
    (TSEG * 16, TSEG * 16 + 0x200),
    (SSEG * 16, SSEG * 16 + 0x10),
]
# Effect slots: DGROUP 0660, 16 × 3C bytes, +2A = target segment.
SLOT_SEGS = [0x660 + i * 0x3C + 0x2A for i in range(16)]

a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, 0x080B), b'\xc3')

CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))


def regions():
    return [bytes(a.mu.mem_read(LOAD_SEG * 16 + lo, hi - lo)) for lo, hi in REGIONS]


base = regions()


def run(seg, off, regs, far=False, budget=20_000_000):
    mu = a.mu
    sentinel = 0xFFF0
    ret = [sentinel, LOAD_SEG + seg] if far else [sentinel]
    sr = {k: v for k, v in regs.items() if k not in ('ds', 'es', 'fs', 'gs')}
    a._setup(ret, sr)
    for k in ('ds', 'es', 'fs', 'gs'):
        if k in regs:
            mu.reg_write(globals()['UC_X86_REG_' + k.upper()], regs[k] + LOAD_SEG)
    mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
    mu.reg_write(UC_X86_REG_IP, off)
    a.fault = None
    try:
        mu.emu_start(a.lin(LOAD_SEG + seg, off), a.lin(LOAD_SEG + seg, sentinel), count=budget)
    except UcError:
        return 'fault'
    if a.fault is not None:
        return 'fault'
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if (cs, ip) != (LOAD_SEG + seg, sentinel):
        op, modrm = mu.mem_read(a.lin(cs, ip), 2)
        if op in (0xF6, 0xF7) and (modrm >> 3) & 7 in (6, 7):
            return 'fault'
        return 'hang'
    return {r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()]) for r in ('ax', 'bx', 'cx', 'dx', 'si', 'di')}


def case(name, writes, seg, off, rets=(), far=False, **regs):
    for (lo, hi), b in zip(REGIONS, base):
        a.mu.mem_write(LOAD_SEG * 16 + lo, b)
    writes = [(DG, 0x91F0, 2, 0), (DG, 0x91F6, 4, rng.getrandbits(32) | 1)] + writes
    enc = []
    for s, o, size, v in writes:
        if isinstance(v, tuple):
            a.mu.mem_write(a.lin(LOAD_SEG + s, o), struct.pack('<H', v[1] + LOAD_SEG))
            enc.append(f'{s:x}:{o:x}:2:s:{v[1]:x}')
        else:
            v &= (1 << 8 * size) - 1
            a.mu.mem_write(a.lin(LOAD_SEG + s, o), v.to_bytes(size, 'little'))
            enc.append(f'{s:x}:{o:x}:{size}:{v:x}')
    before = regions()
    r = run(seg, off, regs, far)
    if isinstance(r, str):
        rv, diff = r, ''
    else:
        # Effect slots store their target's segment; un-relocate it.
        for p in SLOT_SEGS:
            lin = a.lin(LOAD_SEG + DG, p)
            v = struct.unpack('<H', a.mu.mem_read(lin, 2))[0]
            if v >= LOAD_SEG and v - LOAD_SEG in (CSEG, DG, SSEG):
                a.mu.mem_write(lin, struct.pack('<H', v - LOAD_SEG))
        after = regions()
        diff = ' '.join(f'{lo + i:x}:{y:x}' for (lo, _), b0, b1 in zip(REGIONS, before, after)
                        for i, (x, y) in enumerate(zip(b0, b1)) if x != y)
        rv = ' '.join(f'{k}:{r[k]:x}' for k in rets)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    out.write(f'{name} | {" ".join(enc)} | {rg} | {rv} | {diff}\n')


def cls():
    return rng.choice(CLASS_IDS) if rng.random() < 0.85 else rng.getrandbits(8)


def faces():
    vals = [rng.choice([0xFF, rng.getrandbits(8), rng.randint(0, 79)]) for _ in range(9)]
    return [(DG, 0xFD4 + i, 1, v) for i, v in enumerate(vals)]


def cseg_rng():
    """RNG state in the creature segment, sometimes with a table."""
    w = [(CSEG, 0x91F6, 4, rng.getrandbits(32) | 1)]
    if rng.random() < 0.2:
        w += [(CSEG, 0x91F0, 2, ('s', TSEG)), (CSEG, 0x91EE, 2, rng.randrange(0, 0x40, 2)),
              (CSEG, 0x91F2, 2, rng.randint(1, 3)), (CSEG, 0x91F4, 2, rng.choice([2, 8, 0x40]))]
        w += [(TSEG, i * 2, 2, rng.getrandbits(16)) for i in range(0x40)]
    else:
        w += [(CSEG, 0x91F0, 2, 0)]
    return w


def seed_record():
    race = rng.randint(0, 7) | (0x80 if rng.random() < 0.2 else 0)
    return [(SSEG, 0, 4, rng.getrandbits(32) | 1), (SSEG, 4, 1, race), (SSEG, 5, 1, cls()),
            (SSEG, 6, 1, rng.choice([rng.randint(0, 3), rng.randint(0, 30)])),
            (SSEG, 7, 1, rng.choice([0, 1])), (SSEG, 8, 1, rng.getrandbits(8))]


def rec_attrs(seg, rec):
    return [(seg, rec + 0x29 + i, 1, rng.getrandbits(8)) for i in range(8)]


for _ in range(150):
    case('npc_roll_level', [], 0x1194, 0x391F, rets=('ax',))
    case('npc_roll_class', [], 0x1194, 0x3937, rets=('ax',))
    case('npc_roll_sex', [], 0x1194, 0x3965, rets=('ax',))
    race = rng.randint(0, 7)
    case('npc_face', faces(), 0x1194, 0x3829, rets=('ax',), far=True, ax=race)

for _ in range(300):
    level = rng.choice([0, 0, rng.randint(1, 20)])
    klass = rng.choice([0xFFFF, rng.choice(CLASS_IDS)])
    case('npc_seed', [], 0x1194, 0x38B9, es=SSEG, di=0, ax=klass, bx=level)

for _ in range(350):
    in_dgroup = rng.random() < 0.15
    seg, rec = (DG, 0xB000) if in_dgroup else (CSEG, rng.randrange(8) * 0x41E)
    case('expand_npc', seed_record() + faces() + cseg_rng(), 0x1194, 0x351D,
         ds=SSEG, si=0, es=seg, di=rec)

# Seed 0 is a fixed point of the LCG: every face roll is 0, so the original
# spins forever when race × 10 is taken (and finishes when it is free).
for _ in range(10):
    race = rng.randint(0, 7)
    w = seed_record() + faces() + cseg_rng()
    w[0] = (SSEG, 0, 4, 0)
    w[1] = (SSEG, 4, 1, race)
    w.append((DG, 0xFD4 + rng.randrange(9), 1, race * 10 if rng.random() < 0.7 else 0xFF))
    case('expand_npc', w, 0x1194, 0x351D, ds=SSEG, si=0, es=CSEG, di=rng.randrange(8) * 0x41E)

for _ in range(200):
    rec = rng.randrange(8) * 0x41E
    w = [(CSEG, rec + 5, 1, cls()), (CSEG, rec + 6, 1, rng.getrandbits(8))]
    case('npc_xp_value', w, 0x1194, 0x399A, es=CSEG, di=rec)
    w = [(CSEG, rec + 5, 1, cls()), (CSEG, rec + 6, 1, rng.randint(0, 25)),
         (CSEG, rec + 0x40D, 2, rng.choice([0, 0x1000]))] + rec_attrs(CSEG, rec)
    case('derive_record_stats', w, 0x1194, 0x379F, ds=CSEG, si=rec, es=DG)

for _ in range(500):
    rec = rng.randrange(8) * 0x41E
    ty = rng.choice([rng.randint(1, 0x17), rng.randint(1, 0x17), 0x2A, 0x0E, 0x16, 0x17, 0x0C, 0x0F, 5, 0xB, 0xA])
    w = [(DG, 0x1042, 2, ('s', CSEG)), (DG, 0x1020, 2, 0x3000), (DG, 0x1022, 2, ('s', CSEG))]
    case('create_monster', w, 0x1194, 0x8CEE, ax=ty, di=rec)
