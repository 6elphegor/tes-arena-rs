"""Emit test vectors for the character rules (src/character.rs etc.).

Each line: name | writes | regs | returned regs | memory diff
  writes: off:size:value (DGROUP, applied before the call)
  diff:   linear:new for every changed byte in segments 3276..DGROUP end,
          linear being image-relative (segment*16 + offset, unrelocated)
  A call that faults (e.g. divide error) returns "fault" and no diff.
"""
import random, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG

a = Arena(sys.argv[1])
rng = random.Random(1994_07)
out = open(sys.argv[2], 'w')
S = 0x1194
LO, HI = 0x3276 * 16, 0x3BA0 * 16 + 0x10000  # image-relative
P = 0x1A4
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
base_ds = None


def case(name, writes, call, rets=('ax',), **regs):
    global base_ds
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32))] + writes
    # Reset both segments to the loaded state so cases are independent.
    region = lambda: bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))
    if base_ds is None:
        base_ds = region()
    a.mu.mem_write(LOAD_SEG * 16 + LO, base_ds)
    for off, size, v in writes:
        a.mu.mem_write(a.lin(a.ds, off), (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    before = region()
    try:
        r = call(**regs)
    except Fault:
        r = None
    after = region()
    diff = '' if r is None else ' '.join(f'{LO + i:x}:{y:x}' for i, (x, y) in enumerate(zip(before, after)) if x != y)
    w = ' '.join(f'{o:x}:{s}:{v & ((1 << 8 * s) - 1):x}' for o, s, v in writes)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    rv = 'fault' if r is None else ' '.join(f'{k}:{r[k]:x}' for k in rets)
    out.write(f'{name} | {w} | {rg} | {rv} | {diff}\n')


near = lambda off: (lambda **r: a.call_near(S, off, **r))
cls = lambda: rng.choice(CLASS_IDS) if rng.random() < 0.85 else rng.getrandbits(8)
attr_bytes = lambda lo, hi: [(P + 0x29 + i, 1, rng.randint(lo, hi)) for i in range(8)]

for _ in range(300):
    case('to256', [], near(0x5B34), ax=rng.randint(-12800, 12800) & 0xffff)
    case('to100', [], near(0x5B07), ax=rng.getrandbits(16))
    case('to100_u', [], near(0x5AA9), ax=rng.getrandbits(16))
    case('dice', [], near(0x4892), ax=rng.choice([rng.randint(1, 40), rng.randint(1, 400)]))
for _ in range(300):
    race = rng.randint(0, 7) if rng.random() < 0.9 else rng.randint(8, 255)
    case('roll_attributes', [(P + 4, 1, race), (P + 7, 1, rng.choice([0, 1, rng.getrandbits(8)]))],
         near(0x370C), di=P)
    case('attrs_to100', [(0x1CD + i, 1, rng.getrandbits(8)) for i in range(8)], near(0x1007), si=0x1CD, di=0x1CD, cx=8)
    case('attrs_to256', [(0x1CD + i, 1, rng.choice([rng.randint(0, 100), 100, rng.getrandbits(8)])) for i in range(8)],
         near(0x1012), si=0x1CD, di=0x1CD, cx=8)
    case('spell_points', [(P + 5, 1, cls())], near(0x37D3), rets=('ax', 'bx'), si=P, ax=rng.randint(0, 300))
    case('attribute_modifiers', [(P + 5, 1, cls())] + attr_bytes(0, 255), near(0x3621), si=P)
    case('player_derived_stats',
         [(P + 5, 1, cls()), (0x1296, 1, rng.choice([0, 1, 1, rng.getrandbits(8)])), (0x5C8, 2, rng.getrandbits(16))]
         + attr_bytes(0, 100 if rng.random() < 0.8 else 255),
         near(0x1408))
    case('roll_max_hp', [(P + 5, 1, cls()), (P + 0x4F, 2, rng.choice([rng.randint(-6, 6), rng.getrandbits(16)]))],
         near(0x374E), si=P)
    case('roll_hit_die', [(P + 5, 1, cls())], near(0x377B), si=P)
    case('roll_starting_gold', [], near(0x2B87))
    case('create_player_stats',
         [(P + 4, 1, rng.randint(0, 7)), (P + 5, 1, rng.choice(CLASS_IDS)), (P + 7, 1, rng.randint(0, 1)),
          (0x5C8, 2, rng.getrandbits(16))],
         lambda **r: a.run_range(S, 0x2AFA, 0x2B37, **r), rets=())
