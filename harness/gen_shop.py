"""Emit test vectors for shops and potions (src/shop.rs).

Each line: name | writes | regs | returned regs | memory diff
  writes: off:size:value (DGROUP); off:2:s:seg for a segment value,
          which the emulator relocates (Rust sees it unrelocated); and
          Loff:size:value for the stock list segment (LIST below)
  regs:   name:value
  rets:   name:value; "cf" is the carry flag on return
  diff:   linear:new for every changed byte in 3276..DGROUP end and in
          the list segment, linear being image-relative (unrelocated)
  A call that faults returns "fault" and no diff; one that is still
  running after HANG_STEPS instructions returns "hang" (the original
  loops forever there).
"""
import random, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from unicorn.x86_const import *

a = Arena(sys.argv[1])
rng = random.Random(1994_11)
out = open(sys.argv[2], 'w')
DGROUP = 0x3BA0
LIST = 0x5000  # image-relative segment for the stock lists
REGIONS = [(0x3276 * 16, 0x3BA0 * 16 + 0x10000), (LIST * 16, LIST * 16 + 0x10000)]
T, ITEM = 0x13F3, 0x13DA
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
base = None
HANG_STEPS = 2_000_000


class Hang(Exception):
    pass


def snap():
    return [bytes(a.mu.mem_read(LOAD_SEG * 16 + lo, hi - lo)) for lo, hi in REGIONS]


def case(name, writes, call, rets=(), **regs):
    global base
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32)), (0x9030, 2, ('s', LIST))] + writes
    if base is None:
        base = snap()
    for (lo, _), b in zip(REGIONS, base):
        a.mu.mem_write(LOAD_SEG * 16 + lo, b)
    enc = []
    for w in writes:
        off, size, v = w[0], w[1], w[2]
        if isinstance(off, str):  # 'L' + offset: list segment
            o = int(off[1:], 16)
            v &= (1 << 8 * size) - 1
            a.mu.mem_write(a.lin(LIST + LOAD_SEG, o), v.to_bytes(size, 'little'))
            enc.append(f'L{o:x}:{size}:{v:x}')
        elif isinstance(v, tuple):
            a.w16(off, v[1] + LOAD_SEG)
            enc.append(f'{off:x}:2:s:{v[1]:x}')
        else:
            v &= (1 << 8 * size) - 1
            a.mu.mem_write(a.lin(a.ds, off), v.to_bytes(size, 'little'))
            enc.append(f'{off:x}:{size}:{v:x}')
    before = snap()
    hang = False
    try:
        r = call(**regs)
    except Fault:
        r = None
    except Hang:
        r, hang = None, True
    after = snap()
    diff = ''
    if r is not None:
        diff = ' '.join(f'{lo + i:x}:{y:x}' for (lo, _), b0, b1 in zip(REGIONS, before, after)
                        for i, (x, y) in enumerate(zip(b0, b1)) if x != y)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    rv = 'hang' if hang else 'fault' if r is None else ' '.join(f'{k}:{r[k]:x}' for k in rets)
    out.write(f'{name} | {" ".join(enc)} | {rg} | {rv} | {diff}\n')


def near(off):
    def call(**regs):
        a._setup([0xfff0], regs)
        r = a._run(0x1194, off, LOAD_SEG + 0x1194, 0xfff0)
        r['cf'] = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
        return r
    return call


def far(off):
    def call(**regs):
        a._setup([0, 0x0800], regs)
        mu = a.mu
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x1194)
        mu.reg_write(UC_X86_REG_IP, off)
        a.fault = None
        mu.emu_start(a.lin(LOAD_SEG + 0x1194, off), a.lin(0x0800, 0), count=HANG_STEPS)
        if a.fault is not None:
            raise Fault(a.fault)
        if (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)) != (0x0800, 0):
            op, modrm = mu.mem_read(a.lin(mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)), 2)
            if op in (0xF6, 0xF7) and (modrm >> 3) & 7 in (6, 7):
                raise Fault(0)
            raise Hang()
        return {r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()]) for r in ('ax', 'bx', 'cx', 'dx')}
    return call


def player_class():
    v = rng.choice(CLASS_IDS) if rng.random() < 0.9 else rng.getrandbits(8)
    return [(0x1A9, 1, v)]


def keys():
    n = rng.choice([0, 0, rng.randint(1, 12)])
    ks = [rng.choice([rng.randint(10, 60), rng.randint(100, 140), rng.getrandbits(8)]) for _ in range(n)]
    return [(0x1044 + i, 1, k) for i, k in enumerate(ks)] + [(0x1044 + n, 1, 0)]


def shop_state(level):
    return player_class() + keys() + [
        (0x13CC, 1, level),
        (0x12D0, 1, rng.getrandbits(8)), (0x1294, 2, rng.choice([0, rng.randint(0, 250)])),
        (0x1391, 4, rng.choice([rng.randint(0, 1000), rng.getrandbits(32)])),
        (0x12C7, 2, rng.choice([0, 13 * rng.randint(0, 20)])),
        (0x12C9, 2, rng.choice([0x7D0, 0x7D0 + 13 * rng.randint(0, 20)])),
        (0x12DB, 1, rng.randint(0, 30)), (0x12DC, 1, rng.randint(0, 30)),
        (0x13F0, 1, 0), (0x13EF, 1, 0),
    ]


def item():
    ty = rng.choice([rng.randint(0, 17), rng.randint(0, 10), 0x80 + rng.randint(0, 12), rng.getrandbits(8)])
    return [(ITEM + i, 1, rng.getrandbits(8)) for i in range(19)] + [
        (ITEM, 1, ty),
        (ITEM + 6, 2, rng.choice([0, rng.randint(1, 3000), rng.getrandbits(16)])),
        (ITEM + 0xE, 1, rng.choice([0x44, 0x40, 0x41, 0x45, 0x50, 0x60, 0x4C, rng.getrandbits(8)])),
    ]


def templates(off, n):
    w = []
    for i in range(n):
        for j in range(13):
            w.append((f'L{off + 13 * i + j:x}', 1, rng.getrandbits(8)))
        w.append((f'L{off + 13 * i + 7:x}', 4, rng.choice([rng.randint(0, 300), rng.getrandbits(32)])))
    return w


lvl = lambda lo=1, hi=40: rng.randint(lo, hi)

for _ in range(250):
    case('class_can_use', item() + player_class(), near(0x7772), rets=('cf',),
         ax=rng.choice([rng.randint(0, 17), rng.randint(0, 31)]), si=ITEM)
    case('key_is_duplicate', keys() + [(0x12D0, 1, rng.choice([0, rng.randint(10, 140), rng.getrandbits(8)]))],
         near(0xEECB), rets=('cf',))
    n = rng.choice([0, 1, 2, rng.randint(2, 40)])
    lst = rng.choice([0, 0x7D0, 13 * rng.randint(0, 50)])
    case('sort', templates(lst, n) + [(0x13F1, 1, rng.getrandbits(8))], near(0xF37B), bx=lst, dx=n)
    case('potion', [(0x5CC, 2, rng.getrandbits(16)), (0x5CE, 2, rng.getrandbits(16))], far(0xB7FA))
    case('roll_level', [(0x0A22 + 0x25, 1, rng.choice([0, 1, 2, rng.getrandbits(8)]))], near(0x4AA7), rets=('ax',))
    case('price_factors', [(0x137F, 1, rng.choice([4, 6, 0xFF, rng.getrandbits(8)]))], near(0x4AF8),
         ax=rng.choice([rng.randint(1, 25), rng.getrandbits(16)]))
for _ in range(120):
    case('magic_weapon', shop_state(lvl(7)), near(0xEC02))
    case('magic_armor', shop_state(lvl(7)), near(0xECED))
for _ in range(60):
    case('generate_stock', shop_state(rng.choice([lvl(1, 6), lvl(7, 25)])), near(0xE826))
    case('open', shop_state(1) + [(0x12E0, 4, rng.choice([0, rng.getrandbits(32)])),
                                  (0x0A22 + 0x25, 1, rng.choice([0, 1, 2])),
                                  (0x137F, 1, rng.choice([4, 6, 0xFF]))], near(0xFAD9))
for _ in range(300):
    case('magic_item', [(0x13CC, 1, 0), (0x12C9, 2, rng.getrandbits(16)), (0x7109, 1, 0)] +
         [(ITEM + i, 1, rng.getrandbits(8)) for i in range(19)], far(0xB848),
         ax=rng.choice([lvl(1, 6), lvl(7, 40), lvl(1, 255)]),
         bx=rng.choice([0xFF, 0xFF, rng.randint(0, 8), rng.getrandbits(8)]))
