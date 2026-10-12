"""Emit test vectors for src/progression.rs from the original code.

Same line format as gen_rules.py:
  name | writes | regs | returned regs | memory diff
Returned registers also include cf (carry flag), si, and seg (DS after the
call, image-relative). A call that faults returns "fault" and no diff.

Stubs: level_up runs the whole far routine 2686:02D8 with its UI calls
0000:9AF8 (takes AX) and 1194:1034 (level-up screen) patched to RETF.
Both are outside the compared memory region.
"""
import random, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from unicorn.x86_const import UC_X86_REG_EFLAGS, UC_X86_REG_SI, UC_X86_REG_DS

a = Arena(sys.argv[1])
rng = random.Random(1994_08)
out = open(sys.argv[2], 'w')
P = 0x1A4
LO, HI = 0x3276 * 16, 0x3BA0 * 16 + 0x10000  # image-relative
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))

for seg, off in ((0x0000, 0x9AF8), (0x1194, 0x1034)):
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')

region = lambda: bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))
base = region()


def extra(r):
    r['cf'] = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
    r['si'] = a.mu.reg_read(UC_X86_REG_SI)
    r['seg'] = (a.mu.reg_read(UC_X86_REG_DS) - LOAD_SEG) & 0xffff
    return r


def near(seg, off):
    return lambda **r: extra(a.call_near(seg, off, **r))


def far(seg, off):
    def go(**regs):
        a._setup([0, STUB_SEG], regs)
        return extra(a._run(seg, off, STUB_SEG, 0))
    return go


def span(seg, start, end):
    return lambda **r: extra(a.run_range(seg, start, end, **r))


def case(name, writes, call, rets=(), **regs):
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32))] + writes
    a.mu.mem_write(LOAD_SEG * 16 + LO, base)
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


cls = lambda: rng.choice(CLASS_IDS) if rng.random() < 0.85 else rng.getrandbits(8)
level = lambda: rng.randint(0, 12) if rng.random() < 0.8 else rng.getrandbits(8)


def xp():
    return rng.choice([rng.randint(0, 5000), rng.randint(0, 200_000), rng.randint(0, 20_000_000),
                       rng.randint(0, 1 << 28), rng.getrandbits(32)])


for _ in range(400):
    case('check_level_up', [(P + 5, 1, cls()), (P + 6, 1, level()), (0x5AD, 4, xp()),
                            (0xA836, 4, rng.getrandbits(32)), (0xA844, 4, rng.getrandbits(32))],
         near(0x2686, 0x0333), rets=('ax', 'cf'))
    case('level_up', [(P + 5, 1, cls()), (P + 6, 1, level()), (0x5AD, 4, xp()),
                      (0x5C8, 2, rng.getrandbits(16) & (0xffff if rng.random() < 0.3 else 0xffdf)),
                      (0x5CA, 2, rng.getrandbits(16)),
                      (P + 0x4F, 2, rng.choice([rng.randint(-3, 3), rng.getrandbits(16)])),
                      (P + 0x5B, 2, rng.getrandbits(16)), (P + 0x40D, 2, rng.getrandbits(16))],
         far(0x2686, 0x02D8))
    # The record seen through DGROUP (as level-up does) and through 3276,
    # where the intended tables are.
    tier_writes = [(P + 5, 1, cls()), (P + 6, 1, level()),
                   (P + 0x40D, 2, rng.getrandbits(16) & (0xffff if rng.random() < 0.3 else 0xefff))]
    case('level_tier_stats', tier_writes, far(0x2686, 0x03C9), ds=a.ds, si=P)
    case('level_tier_stats', tier_writes, far(0x2686, 0x03C9), ds=LOAD_SEG + 0x3276, si=0x92A0 + P)
    case('player_class_is', [(P + 5, 1, cls())], far(0x2686, 0x02CE), rets=('cf',),
         ax=rng.choice(CLASS_IDS + [rng.getrandbits(8)]))
    case('class_name', [(P + 4, 1, rng.randint(0, 40)), (P + 5, 1, cls()),
                        (P + 0x40D, 2, rng.getrandbits(16))],
         far(0x2686, 0x0645), rets=('seg', 'si'), ds=a.ds, si=P)
    case('award_xp', [(0x5AD, 4, xp()), (0x5CA, 2, rng.getrandbits(16))],
         span(0x2AA1, 0x06D5, 0x06E8), eax=xp())
    case('award_quest_xp', [(P + 6, 1, level()), (0x5AD, 4, xp()), (0x5CA, 2, rng.getrandbits(16))],
         span(0x1194, 0x9099, 0x90B8), cx=rng.choice([500, 50, rng.getrandbits(16)]))
    staff = [(P + 6, 1, level()), (0x5AD, 4, xp()), (0x5C8, 2, rng.getrandbits(16)),
             (0x5CA, 2, rng.getrandbits(16)), (0xF76, 1, rng.choice([0, 0, 1, rng.getrandbits(8)])),
             (0xF77, 1, rng.choice([rng.randint(0, 9), rng.getrandbits(8)])),
             (0xF7A, 1, rng.getrandbits(8)), (0xF7C, 1, rng.choice([0, 1, rng.getrandbits(8)]))] \
        + [(0xF78 + i, 1, rng.getrandbits(8)) for i in range(2)] + [(0xF7D + i, 1, rng.getrandbits(8)) for i in range(4)]
    case('start_staff_piece', staff, far(0x1194, 0xD340), rets=('cf',))
    case('complete_staff_piece', staff, far(0x1194, 0xD363), rets=('cf',))
    case('roll_level_hit_points', [(P + 5, 1, cls()), (P + 6, 1, rng.choice([rng.randint(0, 30), rng.getrandbits(8)])),
                                   (P + 0x59, 2, rng.getrandbits(16)), (P + 0x5B, 2, rng.getrandbits(16))],
         span(0x1194, 0x3893, 0x38B0), si=P)
