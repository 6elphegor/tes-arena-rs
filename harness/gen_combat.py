"""Emit test vectors for src/combat.rs from the original code.

Inventory routines in segment 1194 that combat calls are replaced by stubs
returning chosen values (the Rust side takes the same values as `Gear`),
and UI calls by plain returns.

Each line: name | writes | regs | gear | returned | memory diff
  writes: d|m:off:size:value, into DGROUP or the creature segment; "D"
          writes a segment value (relocated in the emulator)
  regs:   name:value; segment registers hold image-relative segments
  gear:   armor:weapon:gauntlets:bonus:missile (weapon/gauntlets ffff = none)
  returned: ax:value and cf:0/1, or "fault@SEG:OFF" (where it stopped)
  diff:   linear:new over DGROUP's span (3276:0000..3BA0:FFFF) and the
          creature segment's first 4 KiB, image-relative
"""
import random, sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from unicorn.x86_const import UC_X86_REG_EFLAGS, UC_X86_REG_CS, UC_X86_REG_IP

a = Arena(sys.argv[1])
rng = random.Random(1994_10)
out = open(sys.argv[2], 'w')
DS, MSEG, PLAYER = 0x3BA0, 0x5000, 0x01A4
REGIONS = [(0x3276 * 16, 0x3BA0 * 16 + 0x10000), (MSEG * 16, MSEG * 16 + 0x1000)]
SEGREGS = ('ds', 'es', 'fs', 'gs')
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))

def patch(seg, off, code):
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), bytes(code))

# UI calls: critical-hit message and weapon-broken message.
patch(0x292B, 0x047C, [0xCB])
patch(0x2AA1, 0x021B, [0xCB])

# Inventory stubs. Unicorn caches translated code, so the stub code is
# written once and reads its result from a data word right after it
# (CS-relative); only the data changes per case.
def make_stub(off, prefix, suffix):
    """prefix + [data addr] + suffix + retf; data word follows."""
    data = off + len(prefix) + 2 + len(suffix) + 1
    patch(0x1194, off, prefix + [data & 0xff, data >> 8] + suffix + [0xCB])
    return data

ARMOR = make_stub(0x7B1C, [0x2E, 0xA1], [])               # mov ax, cs:[d]
WEAPON = make_stub(0x7510, [0x2E, 0x8B, 0x36], [])        # mov si, cs:[d]
GAUNT = make_stub(0x7449, [0x2E, 0x8B, 0x36], [0x83, 0xFE, 0xFF])  # cmp si,-1: CF = found
BONUS = make_stub(0x39ED, [0x2E, 0xA1], [])               # mov ax, cs:[d]
MISSILE = make_stub(0x78DE, [0x2E, 0x80, 0x3E], [0x01])   # cmp byte cs:[d],1: CF = d < 1

def stubs(g):
    armor, weapon, gaunt, bonus, missile = g
    for d, v in [(ARMOR, armor), (WEAPON, weapon), (GAUNT, gaunt), (BONUS, bonus), (MISSILE, 0 if missile else 1)]:
        a.mu.mem_write(a.lin(LOAD_SEG + 0x1194, d), (v & 0xffff).to_bytes(2, 'little'))

def region():
    return [bytes(a.mu.mem_read(LOAD_SEG * 16 + lo, hi - lo)) for lo, hi in REGIONS]

base = region()

def case(name, writes, run, gear=(0, 0xFFFF, 0xFFFF, 0, 0), **regs):
    writes = [('d', 0x91F0, 2, 0), ('d', 0x91F6, 4, rng.getrandbits(32))] + writes
    for (lo, hi), b in zip(REGIONS, base):
        a.mu.mem_write(LOAD_SEG * 16 + lo, b)
    for kind, off, size, v in writes:
        seg = MSEG if kind == 'm' else DS
        if kind == 'D':
            v += LOAD_SEG
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    stubs(gear)
    before = region()
    emu_regs = {k: (v + LOAD_SEG if k in SEGREGS else v) for k, v in regs.items()}
    emu_regs.setdefault('ds', LOAD_SEG + DS)
    try:
        r = run(**emu_regs)
        r['cf'] = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
    except Fault:
        r = None
        where = f'{a.mu.reg_read(UC_X86_REG_CS) - LOAD_SEG:04X}:{a.mu.reg_read(UC_X86_REG_IP):04X}'
    after = region()
    diff = '' if r is None else ' '.join(
        f'{lo + i:x}:{y:x}' for (lo, _), b0, b1 in zip(REGIONS, before, after)
        for i, (x, y) in enumerate(zip(b0, b1)) if x != y)
    w = ' '.join(f'{k}:{o:x}:{s}:{v & ((1 << 8 * s) - 1):x}' for k, o, s, v in writes)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    gr = ':'.join(f'{x & 0xffff:x}' for x in gear)
    rv = f'fault@{where}' if r is None else f'ax:{r["ax"]:x} cf:{r["cf"]}'
    out.write(f'{name} | {w} | {rg} | {gr} | {rv} | {diff}\n')

def near(off):
    return lambda **r: a.call_near(0x2AA1, off, **r)

def rec(kind, off, fields):
    return [(kind, off + f, s, v) for f, s, v in fields]

def combatant(kind, off, creature=None):
    """Random combat record fields at kind:off."""
    if creature is None:
        creature = rng.random() < 0.5
    flags = rng.getrandbits(16) & ~0x1000 | (0x1000 if creature else 0)
    cls = rng.choice(CLASS_IDS) if rng.random() < 0.8 else rng.getrandbits(8)
    return rec(kind, off, [
        (4, 1, rng.randint(0, 7) if rng.random() < 0.7 else rng.getrandbits(8)),
        (5, 1, cls),
        (6, 1, rng.randint(1, 30) if rng.random() < 0.8 else rng.getrandbits(8)),
        (9, 1, rng.randint(0, 10)), (0xA, 1, rng.randint(10, 40)),
        (0x39, 2, rng.randint(-5, 10)),
        (0x47, 2, rng.randint(-128, 127)), (0x49, 2, rng.randint(-128, 127)),
        (0x57, 2, rng.choice([50, rng.randint(-60, 60)])),
        (0x40D, 2, flags),
    ])

def pair():
    """(attacker kind, off, defender kind, off) for player vs. other."""
    other = rng.choice([0, 0x100, 0x480])
    if rng.random() < 0.5:
        return 'd', PLAYER, 'm', other
    return 'm', other, 'd', PLAYER

SEG = {'d': DS, 'm': MSEG}

def gear():
    return (rng.randint(-60, 300), 0xFFFF, 0xFFFF, rng.randint(0, 5), rng.random() < 0.3)

def creature_target():
    ty = rng.choice([7, 10, 12, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, rng.getrandbits(8)])
    return [('D', 0x1042, 2, MSEG)] + rec('m', 0, [
        (4, 1, ty), (0x40D, 2, rng.getrandbits(16) | (0x1000 if rng.random() < 0.8 else 0))])

def item(kind, off):
    lo = rng.randint(0, 20)
    hi = lo + rng.choice([0, 1, rng.randint(1, 30)])
    return rec(kind, off, [
        (0, 1, rng.getrandbits(8)),
        (4, 1, lo), (5, 1, hi & 0xff),
        (6, 2, rng.choice([rng.randint(0, 3000), rng.getrandbits(16)])),
        (0xE, 1, rng.getrandbits(8)),
        (0x10, 1, rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 0xFF, rng.getrandbits(8)])),
    ])

for _ in range(400):
    case('percent', [], lambda **r: a.call_near(0x1194, 0x5572, **r))
    case('hit_location', [], near(0x0687))

    ak, ao, dk, do = pair()
    case('hit_base', combatant(ak, ao) + combatant(dk, do), near(0x0638),
         fs=SEG[ak], si=ao, es=SEG[dk], di=do)

    ak, ao, dk, do = pair()
    case('hit_chance', combatant(ak, ao) + combatant(dk, do) + [('d', 0x129A, 2, rng.randint(0, 7))],
         near(0x060F), gear(), fs=SEG[ak], si=ao, es=SEG[dk], di=do)

    ak, ao, dk, do = pair()
    defn = combatant(dk, do) + rec(dk, do, [(5, 1, rng.choice([0x4C, 0xC9, rng.choice(CLASS_IDS)]))])
    case('roll_to_hit', combatant(ak, ao) + defn + [
            ('d', 0x1402, 2, do), ('d', 0x05C6, 2, rng.choice([0, rng.randint(0, 100)])),
            ('d', 0x01AA, 1, rng.randint(1, 30) if rng.random() < 0.9 else rng.getrandbits(8))],
         near(0x058B), gear(), ax=rng.randint(0, 7), fs=SEG[ak], si=ao, es=SEG[dk], di=do)

    ak, ao, _, _ = pair()
    crit = rec(ak, ao, [(5, 1, rng.choice([0x4C, 0x4D, 0xCA, 0xCB, 0xE5, 0xC7] + CLASS_IDS)),
                        (6, 1, rng.randint(1, 40) if rng.random() < 0.9 else rng.getrandbits(8))])
    case('critical_multiplier', crit, near(0x0852), gear(), ds=SEG[ak], si=ao)

    ak, ao, dk, do = pair()
    io = 0xF000 if ak == 'd' else 0x0800
    g = list(gear())
    choice = rng.random()
    if choice < 0.6:
        g[1] = io
    elif choice < 0.8:
        g[2] = io
    att = combatant(ak, ao) + (rec(ak, ao, [(5, 1, 0x0E)]) if rng.random() < 0.3 else [])
    case('weapon_damage', att + combatant(dk, do) + creature_target() + item(ak, io) + [
            ('d', 0x1400, 2, ao), ('d', 0x1402, 2, do), ('m', 0x1402, 2, rng.choice([0, do])),
            ('d', 0x05CA, 2, rng.getrandbits(16)), ('d', 0x01FD, 2, rng.randint(0, 100))],
         near(0x04A5), tuple(g), ax=rng.choice([1, 1, 3]), ds=SEG[ak], es=rng.choice([DS, MSEG]))

    case('creature_damage_taken', creature_target() + [('d', 0x0D49, 1, rng.randint(0, 7))],
         near(0x0325), ax=rng.getrandbits(16))

    ak, ao, _, _ = pair()
    case('creature_damage', combatant(ak, ao) + rec(ak, ao, [(9, 1, rng.getrandbits(8)), (0xA, 1, rng.getrandbits(8))]),
         lambda **r: a.run_range(0x2AA1, 0x0115, 0x0127, **r), ds=SEG[ak], si=ao)

# Weapon wear against special creatures, with materials around each
# threshold (types 0x11..0x13 need type-0x11+3; 0x14.. use a table, and
# type 0x16 with material 2 is a special case).
for _ in range(400):
    ty = rng.choice([0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 7, 10])
    need = ty - 0x11 + 3 if ty <= 0x13 else rng.randint(0, 8)
    mat = rng.choice([need - 1, need, need + 1, 2, 0xFF]) & 0xff
    io = rng.choice([(DS, 'd', 0xF000), (MSEG, 'm', 0x0800)])
    target = [('D', 0x1042, 2, MSEG)] + rec('m', 0, [
        (4, 1, ty), (0x40D, 2, rng.getrandbits(16) | (0x1000 if rng.random() < 0.9 else 0))])
    wpn = rec(io[1], io[2], [
        (0, 1, rng.getrandbits(8)), (6, 2, rng.randint(0, 3000)),
        (0xE, 1, rng.getrandbits(8) | (1 if rng.random() < 0.85 else 0)), (0x10, 1, mat)])
    case('weapon_wear', target + wpn + [('d', 0x05CA, 2, rng.getrandbits(16)), ('m', 0x05CA, 2, rng.getrandbits(16))],
         near(0x017C), ax=rng.randint(0, 60), ds=io[0], si=io[2], es=rng.choice([DS, MSEG]))
