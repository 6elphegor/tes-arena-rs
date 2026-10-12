"""Emit test vectors for loot and item generation (src/loot.rs).

Each line: name | writes | regs | returned regs | memory diff
  writes: off:size:value (DGROUP), or off:2:s:seg for a segment value,
          which the emulator relocates (Rust sees it unrelocated)
  regs:   name:value; "cf" is the carry flag on entry
  diff:   linear:new for every changed byte in segments 3276..DGROUP end,
          linear being image-relative (segment*16 + offset, unrelocated)
  A call that faults returns "fault" and no diff.
"""
import random, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from unicorn.x86_const import UC_X86_REG_EFLAGS

a = Arena(sys.argv[1])
rng = random.Random(1994_09)
out = open(sys.argv[2], 'w')
LO, HI = 0x3276 * 16, 0x3BA0 * 16 + 0x10000
DGROUP = 0x3BA0
base = None
T, ITEM = 0x13F3, 0x13DA
REC = 0xB000  # a creature record somewhere in DGROUP BSS


def region():
    return bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))


def case(name, writes, call, rets=('ax',), **regs):
    global base
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32))] + writes
    if base is None:
        base = region()
    a.mu.mem_write(LOAD_SEG * 16 + LO, base)
    enc = []
    for w in writes:
        off, size, v = w[0], w[1], w[2]
        if isinstance(v, tuple):  # ('s', seg): relocated segment value
            a.w16(off, v[1] + LOAD_SEG)
            enc.append(f'{off:x}:2:s:{v[1]:x}')
        else:
            v &= (1 << 8 * size) - 1
            a.mu.mem_write(a.lin(a.ds, off), v.to_bytes(size, 'little'))
            enc.append(f'{off:x}:{size}:{v:x}')
    before = region()
    try:
        r = call(**regs)
    except Fault:
        r = None
    after = region()
    diff = '' if r is None else ' '.join(
        f'{LO + i:x}:{y:x}' for i, (x, y) in enumerate(zip(before, after)) if x != y)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    rv = 'fault' if r is None else ' '.join(f'{k}:{r[k]:x}' for k in rets)
    out.write(f'{name} | {" ".join(enc)} | {rg} | {rv} | {diff}\n')


def with_cf(fn):
    """Wrap a call so a 'cf' register sets the carry flag on entry."""
    def call(cf=0, **regs):
        a._setup([0xfff0], regs)
        a.mu.reg_write(UC_X86_REG_EFLAGS, 0x2 | (cf & 1))
        return a._run(0x1194, fn, LOAD_SEG + 0x1194, 0xfff0)
    return call


near = lambda seg, off: (lambda **r: a.call_near(seg, off, **r))
far = lambda seg, off: (lambda **r: a.call_far(seg, off, **r))
lvl = lambda lo=1, hi=40: rng.randint(lo, hi)


def template(magic=None, weapon=None):
    """Random but plausible template contents."""
    return [
        (T + 0, 1, rng.getrandbits(8)),
        (T + 1, 1, rng.choice([0, 1]) if magic is None else magic),
        (T + 2, 1, rng.choice([0, 1]) if weapon is None else weapon),
        (T + 3, 1, rng.choice([0, 1, 2, rng.getrandbits(8)])),
        (T + 4, 1, rng.choice([rng.randint(0, 7), rng.randint(0, 7), 0xFF, rng.getrandbits(8)])),
        (T + 5, 1, rng.randint(0, 17)),
        (T + 6, 1, rng.choice([rng.randint(0, 13), 0xFF])),
        (T + 7, 4, rng.choice([rng.randint(0, 200), rng.randint(0, 60000), rng.getrandbits(32)])),
        (T + 11, 1, rng.getrandbits(8)),
        (T + 12, 1, rng.getrandbits(8)),
    ]


common = lambda: [(0x12D0, 1, rng.getrandbits(8)), (0x1294, 2, rng.getrandbits(16)),
                  (0x1391, 4, rng.choice([rng.randint(0, 1000), rng.getrandbits(32)]))]

for _ in range(250):
    # Entry points.
    case('loot_weapon', common() + [(0x13CC, 1, 0)], far(0x1194, 0xE8B2), rets=(),
         ax=lvl(), bx=rng.choice([0xFF, 0xFF, rng.randint(0, 17)]))
    case('loot_armor', common() + [(0x101E, 1, rng.choice([0xFF, 0xFF, rng.randint(0, 2)]))],
         far(0x1194, 0xE889), rets=(), ax=lvl(), bx=rng.choice([0xFF, 0xFF, rng.randint(0, 10)]))
    case('loot_magic_weapon', common(), far(0x1194, 0xE922), rets=(),
         ax=lvl(7), bx=rng.choice([0xFFFF, 0xFFFF, rng.randint(0, 17)]))
    case('loot_magic_armor', common(), far(0x1194, 0xE94E), rets=(),
         ax=lvl(7), bx=rng.choice([0xFFFF, 0xFFFF, rng.randint(0, 10)]))
    # Building blocks with random templates.
    case('roll_material', common() + template(), near(0x1194, 0xEDBC), rets=())
    case('roll_enchantment', common() + template() + [(0x13CC, 1, lvl(7))],
         with_cf(0xEE1D), rets=(), cf=rng.getrandbits(1))
    case('set_condition', common() + template(), near(0x1194, 0xF312), rets=(),
         si=ITEM, ax=rng.choice([0x80, 0x40, rng.getrandbits(16)]))
    case('finish_weapon', common() + template(), near(0x1194, 0xF031), rets=(), si=ITEM)
    case('finish_armor', common() + template(weapon=0) + [(T + 5, 1, rng.randint(0, 10))],
         near(0x1194, 0xEEE2), rets=(), si=ITEM)
    kind = rng.randint(0, 2)
    prices, mins = [(0x24F4, 0x2725), (0x24E9, 0x27C7), (0x24DE, 0x2877)][kind]
    case(f'pick_armor', common() + template() + [(0x13CC, 1, lvl()),
         (0x86B0, 1, rng.choice([0xFF, 0xFF, rng.randint(0, 10)]))],
         near(0x1194, 0xE9C4), rets=(), di=prices, bx=mins)
    # Corpse loot (24FA).
    case('d100', [], near(0x24FA, 0x0300))
    case('roll_loot_flags', [(REC + 4, 1, rng.randint(0, 23))],
         lambda **r: a.run_range(0x24FA, 0x0009, 0x003C, **r), rets=(), si=REC)
    case('roll_corpse_gold', [(0x4EFE, 2, ('s', DGROUP)), (0x4F00, 2, REC),
                              (REC + 4, 1, rng.randint(0, 23)), (REC + 6, 1, rng.getrandbits(8))],
         near(0x24FA, 0x009C), rets=())
    case('roll_creature_gold', [(REC + 5, 1, rng.getrandbits(8))], far(0x24FA, 0x004D), rets=(), si=REC)
