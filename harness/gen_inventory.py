"""Emit test vectors for src/inventory.rs.

Each line: name | writes | regs | returned regs | memory diff
  writes: lin:size:value      bytes at an image-relative linear address
          inv:lin:seed         40 item slots filled by `fill_slots(seed)`
          blob:lin:hex         raw bytes
          seg:lin:value        a word holding a segment (relocated here)
  regs:   name:value, segment registers image-relative
  returns: registers plus cf/sf where the routine returns a flag
  diff:   lin:new for every changed byte from 3276:0000 to DGROUP's end
"""
import random, struct, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from unicorn.x86_const import *

a = Arena(sys.argv[1])
rng = random.Random(0x1994_0B17)
out = open(sys.argv[2], 'w')
S = 0x1194
DG = 0x3BA0
ALT = 0x4AF7           # a record in DGROUP's BSS, as its own segment
P, PSLOTS = 0x1A4, 0x212
LO, HI = 0x3276 * 16, DG * 16 + 0x10000


def xorshift(x):
    x ^= (x << 13) & 0xFFFFFFFF
    x ^= x >> 17
    x ^= (x << 5) & 0xFFFFFFFF
    return x


def fill_slots(seed):
    """40 slots of 19 bytes; mirrored by `fill_slots` in tests/inventory.rs."""
    x = seed | 1
    def nxt():
        nonlocal x
        x = xorshift(x)
        return x
    out = bytearray(40 * 19)
    for s in range(40):
        if nxt() % 5 == 0:
            continue
        b = bytearray(nxt() & 0xFF for _ in range(19))
        kind = nxt() % 8
        if kind <= 2:
            ty, cat = nxt() % 18, 1
        elif kind <= 4:
            ty, cat = nxt() % 11, 0
        elif kind == 5:
            ty, cat = 0x80 | (nxt() % 0x10), nxt() % 4
        elif kind == 6:
            ty, cat = b[0], nxt() % 4
        else:
            ty, cat = nxt() % 4, 2
        flags = b[14] & 0xF3 | cat << 2
        flags = flags | 0x40 if nxt() % 16 else flags & 0xBF
        if nxt() % 3 == 0:
            b[6] = b[7] = 0
        b[0], b[14] = ty, flags
        out[s * 19:(s + 1) * 19] = b
    return bytes(out)


base_mem = None
REGS = ('ax', 'bx', 'cx', 'si', 'di')


def run(seg, off, far, regs, cf):
    mu = a.mu
    if far:
        a._setup([0xFFF0, LOAD_SEG + seg], regs)
    else:
        a._setup([0xFFF0], regs)
    mu.reg_write(UC_X86_REG_EFLAGS, 0x2 | (1 if cf else 0))
    r = a._run(seg, off, LOAD_SEG + seg, 0xFFF0)
    fl = mu.reg_read(UC_X86_REG_EFLAGS)
    r.update(si=mu.reg_read(UC_X86_REG_SI), di=mu.reg_read(UC_X86_REG_DI), cf=fl & 1, sf=(fl >> 7) & 1)
    return r


def case(name, writes, entry, rets, cf=False, **regs):
    global base_mem
    seg, off, far = entry
    region = lambda: bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))
    if base_mem is None:
        base_mem = region()
    a.mu.mem_write(LOAD_SEG * 16 + LO, base_mem)
    for w in writes:
        kind = w[0]
        if kind == 'inv':
            a.mu.mem_write(LOAD_SEG * 16 + w[1], fill_slots(w[2]))
        elif kind == 'blob':
            a.mu.mem_write(LOAD_SEG * 16 + w[1], w[2])
        elif kind == 'seg':
            a.mu.mem_write(LOAD_SEG * 16 + w[1], struct.pack('<H', LOAD_SEG + w[2]))
        else:
            lin, size, v = w
            a.mu.mem_write(LOAD_SEG * 16 + lin, (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    emu_regs = {k: (LOAD_SEG + v if k in ('ds', 'es', 'fs', 'gs') else v) for k, v in regs.items()}
    emu_regs.setdefault('ds', a.ds)
    emu_regs.setdefault('es', a.ds)
    before = region()
    try:
        r = run(seg, off, far, emu_regs, cf)
    except Fault:
        r = None
    after = region()
    diff = '' if r is None else ' '.join(f'{LO + i:x}:{y:x}' for i, (x, y) in enumerate(zip(before, after)) if x != y)
    ws = []
    for w in writes:
        if w[0] == 'inv':
            ws.append(f'inv:{w[1]:x}:{w[2]:x}')
        elif w[0] == 'blob':
            ws.append(f'blob:{w[1]:x}:{w[2].hex()}')
        elif w[0] == 'seg':
            ws.append(f'seg:{w[1]:x}:{w[2]:x}')
        else:
            ws.append(f'{w[0]:x}:{w[1]}:{w[2] & ((1 << 8 * w[1]) - 1):x}')
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    if 'cf' in rets:
        rg += f' cfin:{int(cf)}'
    rv = 'fault' if r is None else ' '.join(f'{k}:{r[k]:x}' for k in rets)
    out.write(f'{name} | {" ".join(ws)} | {rg} | {rv} | {diff}\n')


lin = lambda seg, off: seg * 16 + off
near = lambda off: (S, off, False)
far = lambda off: (S, off, True)


def records():
    """(segment, record offset, writes) for a player or a BSS record."""
    seed = rng.getrandbits(32)
    if rng.random() < 0.6:
        seg, rec = DG, P
    else:
        seg, rec = ALT, 0
    w = [('inv', lin(seg, rec + 0x6E), seed)]
    # The player's slots too, for routines hardwired to DGROUP 0212.
    if seg != DG:
        w.append(('inv', lin(DG, PSLOTS), rng.getrandbits(32)))
    return seg, rec, w


def slot_ptr(rec):
    return rec + 0x6E + 19 * rng.randrange(40)


def record_fields(seg, rec):
    return [(lin(seg, rec + 4), 1, rng.choice([0, 1, 2, 3, 4, 5, 6, 7, rng.getrandbits(8)])),
            (lin(seg, rec + 6), 1, rng.getrandbits(8)),
            (lin(seg, rec + 0x40D), 2, rng.choice([0, 0, 0, 0x1000, rng.getrandbits(16)]))]


for _ in range(150):
    seg, rec, w = records()
    # Pointers stay in areas the case controls: elsewhere DGROUP holds
    # relocated segment words, which differ between emulator and port.
    si = rng.choice([rec + 0x6E, rec, rec + 0x6E + rng.randrange(0, 19 * 39)])
    case('count_items', w, near(0x7472), ('ax', 'cf'), ds=seg, si=si)
    case('base_type', [], near(0x7596), ('ax', 'cf'), ax=rng.getrandbits(8))

    seg, rec, w = records()
    ty = rng.choice([rng.randrange(18), rng.randrange(11), rng.getrandbits(16)])
    start = rec + 0x6E + 19 * rng.randrange(40)
    case('find_item', w, near(0x75D3), ('cf', 'si', 'cx'), ds=seg, si=start,
         cx=rng.choice([0, rng.randrange(1, 41), 40]), ax=ty, bx=rng.choice([0, 1, 2, 3, 5]))
    case('find_equipped', w, near(0x745A), ('cf', 'si'), cf=rng.random() < 0.5, ds=seg, si=start,
         cx=rng.choice([0, rng.randrange(1, 41), 40]), ax=ty, bx=rng.choice([0, 1, 2, 3]))
    case('find_player_equipped', w, far(0x7449), ('cf', 'si'), ds=seg,
         si=rng.choice([rec, rec + 0x6E, rec + 0x6E + rng.randrange(0, 19 * 39)]), ax=rng.choice([1, rng.randrange(11)]), bx=rng.choice([0, 1, 2]))

    seg, rec, w = records()
    case('equipped_shield', w, far(0x74A4), ('cf', 'si'), ds=seg, si=rec + 0x6E)
    case('shield_at', w, far(0x74DA), ('sf', 'si'), ds=seg, si=rec + 0x6E, ax=rng.choice([rng.randrange(8), rng.getrandbits(16)]))
    case('armor_at', w, far(0x7B1C), ('ax',), ds=seg, si=rec, ax=rng.choice([rng.randrange(8), rng.getrandbits(8)]))
    case('equipped_weapon', w, far(0x7510), ('si',), ds=seg, si=rec)
    case('has_missile_weapon', w, far(0x78DE), ('cf',), ds=seg, si=rec)
    case('weapon_bonus', w + record_fields(seg, rec), far(0x39ED), ('ax',), ds=seg, si=rec)
    case('nth_usable_item', w + [(lin(seg, rec + 5), 1, rng.getrandbits(8))], far(0x7AE7), ('si',),
         ds=seg, si=rec, ax=rng.choice([0, 1, 2, rng.randrange(10), 0xFFFF]))
    case('free_slot', w, far(0x5EB1), ('cf', 'si', 'bx'), ds=seg, si=rec)

    seg, rec, w = records()
    it = slot_ptr(rec)
    case('class_wears_no_armor', [], far(0x7757), ('cf',), ax=rng.getrandbits(16))
    case('class_can_use', w, far(0x776E), ('cf',), ds=seg, si=it,
         ax=rng.choice([rng.randrange(18), rng.randrange(18), rng.getrandbits(8)]))
    case('item_has_effect', w, far(0x7A86), ('cf',), ds=seg, si=it)
    case('condition_name', [('inv', lin(DG, PSLOTS), rng.getrandbits(32))], near(0x7BE7), ('si',),
         si=slot_ptr(P))

    # Adding to a record in ALT from the loot item at DGROUP 13DA.
    full = rng.random() < 0.2
    case('add_item',
         [('inv', lin(ALT, 0x6E), rng.getrandbits(32)), ('seg', lin(DG, 0x4EFE), ALT), (lin(DG, 0x4F00), 2, 0),
          ('blob', lin(DG, 0x13DA), bytes(rng.getrandbits(8) for _ in range(19)))]
         + ([(lin(ALT, 0x6E + 19 * i + 14), 1, 0x40) for i in range(40)] if full else []),
         (0x24FA, 0x02C4, False), (), si=0x13DA)

    idx = rng.choice([rng.randrange(40), rng.randrange(40), 39])
    case('remove_item', [('inv', lin(ALT, 0x6E), rng.getrandbits(32))], far(0x7889), (),
         ds=ALT, si=0x6E + 19 * idx, ax=idx)

    case('player_armor_class',
         [('inv', lin(DG, PSLOTS), rng.getrandbits(32)), (lin(DG, P + 0x4B), 2, rng.choice([rng.randint(-128, 127), rng.getrandbits(16)]))],
         near(0x7719), (), es=DG)

for _ in range(150):
    # Edge cases random inventories rarely reach: Knights and their
    # neighbours with each armour kind, and stacks with counts near 0.
    seg, rec, w = records()
    it = slot_ptr(rec)
    kind = rng.randrange(4)
    case('class_can_use', w + [(lin(seg, it), 1, rng.randrange(7)), (lin(seg, it + 6), 2, 50),
                               (lin(seg, it + 14), 1, 0x40 | kind << 4)],
         far(0x776E), ('cf',), ds=seg, si=it, ax=rng.choice([15, 16, 17]))
    idx = rng.randrange(40)
    case('remove_item', [('inv', lin(ALT, 0x6E), rng.getrandbits(32)),
                         (lin(ALT, 0x6E + 19 * idx + 14), 1, 0x40 | 3 << 2 | rng.choice([0, 0x80])),
                         (lin(ALT, 0x6E + 19 * idx + 1), 2, rng.choice([0, 1, 2, 0x8000]))],
         far(0x7889), (), ds=ALT, si=0x6E + 19 * idx, ax=idx)

for _ in range(150):
    # Combat's to-hit chance (2AA1:060F) with no stubs: the defender's
    # armour comes from its real inventory through 1194:7B1C.
    def fields(seg, rec):
        return [(lin(seg, rec + 4), 1, rng.randrange(8)), (lin(seg, rec + 6), 1, rng.randrange(30)),
                (lin(seg, rec + 0x40D), 2, rng.choice([0, 0x1000])),
                (lin(seg, rec + 0x47), 2, rng.randint(-128, 127)), (lin(seg, rec + 0x49), 2, rng.randint(-128, 127)),
                (lin(seg, rec + 0x57), 2, 50)]
    case('hit_chance', [('inv', lin(ALT, 0x6E), rng.getrandbits(32)), (lin(DG, 0x129A), 2, rng.randrange(8))]
         + fields(DG, P) + fields(ALT, 0) + [(lin(DG, P + 6), 1, rng.randrange(30))],
         (0x2AA1, 0x060F, False), ('ax',), fs=DG, si=P, es=ALT, di=0)

for _ in range(150):
    # memmove within and between two areas, overlapping and onto itself.
    areas = [(DG, 0x6000), (ALT, 0x100)]
    blobs = [('blob', lin(sg, o), bytes(rng.getrandbits(8) for _ in range(0x140))) for sg, o in areas]
    ss, so = rng.choice(areas)
    ds_, do = rng.choice(areas)
    si = so + rng.randrange(0x40, 0x100)
    di = rng.choice([si, si + rng.randint(-40, 40), do + rng.randrange(0x40, 0x100)]) if ds_ == ss else do + rng.randrange(0x40, 0x100)
    case('memmove', blobs, (0, 0x7732, True), ('si', 'di'), ds=ss, si=si, es=ds_, di=di, cx=rng.randrange(0, 60))
