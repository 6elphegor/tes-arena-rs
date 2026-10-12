"""Emit test vectors for src/effects.rs from the original code.

Handlers that are not ported are replaced by stubs that record their
calls: the kill handler 2AA1:06A0 (returns a chosen carry flag) and the
player-death handler 2686:02B4. Inventory queries are stubs returning
chosen values (1194:7449 armour at the hit location, 1194:7C69
enchantments 84/85/88), and UI calls return at once (the formatter
0000:8D14, the broken-weapon message 2AA1:021B).

Each line: name | writes | regs | env | returned | hooks | memory diff
  writes: k:off:hexbytes, or K:off:size:value for segment values
          (relocated in the emulator), applied in order. d/D DGROUP, m/M
          the creature segment (5000), o another segment (5800)
  regs:   name:value; segment registers hold image-relative segments
  env:    kill_cf:armoured:has84:has85:has88 (0/1)
  returned: ax bx si cf di fs, or "fault@SEG:OFF"
  hooks:  kills:last_seg:last_off deaths reflections:last_si
  diff:   linear:new over 3276:0000..3BA0:FFFF, 5000:0000..1400 and
          5800:0000..0600, image-relative; each effect slot's segment
          word is un-relocated first
"""
import random, sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from unicorn.x86_const import (UC_X86_REG_EFLAGS, UC_X86_REG_CS, UC_X86_REG_IP,
                               UC_X86_REG_SI, UC_X86_REG_AX, UC_X86_REG_BX,
                               UC_X86_REG_DI, UC_X86_REG_FS)


class FxArena(Arena):
    def _intr(self, mu, intno, _):
        # INT 3 in 2EBD:0696 is a no-op under DOS's default handler.
        if intno == 3:
            return
        self.fault = intno
        mu.emu_stop()


a = FxArena(sys.argv[1])
rng = random.Random(1994_11)
out = open(sys.argv[2], 'w')
DS, MSEG, OSEG, PLAYER = 0x3BA0, 0x5000, 0x5800, 0x01A4
SLOTS, SLOT_SIZE = 0x0660, 0x3C
REGIONS = [(0x3276 * 16, 0x3BA0 * 16 + 0x10000), (MSEG * 16, MSEG * 16 + 0x1400), (OSEG * 16, OSEG * 16 + 0x600)]
SEGREGS = ('ds', 'es', 'fs', 'gs')
KSEG = {'d': DS, 'D': DS, 'm': MSEG, 'M': MSEG, 'o': OSEG}
CLASS_IDS = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))


def patch(seg, off, code):
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), bytes(code))


def cs_ref(addr):
    return [addr & 0xff, addr >> 8]


# UI: the formatter and the broken-weapon message.
patch(0x0000, 0x8D14, [0xCB])
patch(0x2AA1, 0x021B, [0xCB])

# Stubs keep their data right after their code (unicorn caches code, so
# only data changes between cases).
KILL = 0x06A0
K_CNT, K_SI, K_DS, K_RES = KILL + 22, KILL + 24, KILL + 26, KILL + 28
patch(0x2AA1, KILL,
      [0x2E, 0xFF, 0x06] + cs_ref(K_CNT) +           # inc word cs:[cnt]
      [0x2E, 0x89, 0x36] + cs_ref(K_SI) +            # mov cs:[si'], si
      [0x2E, 0x8C, 0x1E] + cs_ref(K_DS) +            # mov cs:[ds'], ds
      [0x2E, 0x80, 0x3E] + cs_ref(K_RES) + [0x01] +  # cmp byte cs:[res], 1
      [0xCB])
DEATH = 0x02B4
D_CNT = DEATH + 6
patch(0x2686, DEATH, [0x2E, 0xFF, 0x06] + cs_ref(D_CNT) + [0xCB])
# 1194:7449: SI = data; CF = SI < FFFF (an item is worn there).
GAUNT = 0x7449
G_SI = GAUNT + 9
patch(0x1194, GAUNT, [0x2E, 0x8B, 0x36] + cs_ref(G_SI) + [0x83, 0xFE, 0xFF, 0xCB])
# 1194:7C69: CF = enchantment AL present (its data byte is 0).
ENCH = 0x7C69
E84, E85, E88 = ENCH + 29, ENCH + 30, ENCH + 31
patch(0x1194, ENCH,
      [0x3C, 0x85, 0x74, 0x0B,                        # cmp al,85; je A
       0x3C, 0x88, 0x74, 0x0E,                        # cmp al,88; je B
       0x2E, 0x80, 0x3E] + cs_ref(E84) + [0x01, 0xCB,  # cmp cs:[84],1; retf
      0x2E, 0x80, 0x3E] + cs_ref(E85) + [0x01, 0xCB,   # A
      0x2E, 0x80, 0x3E] + cs_ref(E88) + [0x01, 0xCB])  # B


# 0000:27E5 (AX=19h after a reflection): count calls, keep SI.
REFL = 0x27E5
R_CNT, R_SI = REFL + 11, REFL + 13
patch(0x0000, REFL, [0x2E, 0xFF, 0x06] + cs_ref(R_CNT) + [0x2E, 0x89, 0x36] + cs_ref(R_SI) + [0xCB])


def setenv(env):
    kill_cf, armoured, h84, h85, h88 = env
    w = lambda seg, off, b: a.mu.mem_write(a.lin(LOAD_SEG + seg, off), bytes(b))
    w(0x2AA1, K_CNT, [0, 0, 0, 0, 0, 0, 0 if kill_cf else 1])
    w(0x2686, D_CNT, [0, 0])
    w(0x1194, G_SI, [0x00, 0x01] if armoured else [0xFF, 0xFF])
    w(0x1194, E84, [0 if h84 else 1, 0 if h85 else 1, 0 if h88 else 1])
    w(0x0000, R_CNT, [0, 0, 0, 0])


def hooks():
    r = lambda seg, off: struct.unpack('<H', a.mu.mem_read(a.lin(LOAD_SEG + seg, off), 2))[0]
    kseg = (r(0x2AA1, K_DS) - LOAD_SEG) & 0xffff if r(0x2AA1, K_CNT) else 0
    return (f'{r(0x2AA1, K_CNT):x}:{kseg:x}:{r(0x2AA1, K_SI):x} {r(0x2686, D_CNT):x} '
            f'{r(0x0000, R_CNT):x}:{r(0x0000, R_SI):x}')


def region():
    imgs = [bytearray(a.mu.mem_read(LOAD_SEG * 16 + lo, hi - lo)) for lo, hi in REGIONS]
    # Un-relocate every slot's target segment (zero means none). The same
    # transform applies before and after, so a slot changing from free to
    # used or back compares like the port.
    d = imgs[0]
    base = DS * 16 - REGIONS[0][0]
    for i in range(16):
        s = base + SLOTS + i * SLOT_SIZE
        v = struct.unpack_from('<H', d, s + 0x2A)[0]
        if v:
            struct.pack_into('<H', d, s + 0x2A, (v - LOAD_SEG) & 0xffff)
    return imgs


base = [bytes(b) for b in region()]


def case(name, writes, run, env=(0, 0, 0, 0, 0), **regs):
    writes = [('d', 0x91F0, 2, 0), ('d', 0x91F6, 4, rng.getrandbits(32) | 1)] + writes
    for (lo, hi), b in zip(REGIONS, base):
        a.mu.mem_write(LOAD_SEG * 16 + lo, b)
    for kind, off, size, v in writes:
        if kind in 'DM':
            v += LOAD_SEG
        a.mu.mem_write(a.lin(LOAD_SEG + KSEG[kind], off), (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    setenv(env)
    before = region()
    emu_regs = {k: (v + LOAD_SEG if k in SEGREGS else v) for k, v in regs.items()}
    emu_regs.setdefault('ds', LOAD_SEG + DS)
    if 'ebx_seg' in emu_regs:
        emu_regs['ebx'] = ((emu_regs.pop('ebx_seg') + LOAD_SEG) << 16) | emu_regs.pop('ebx_off')
    try:
        run(**emu_regs)
        rv = (f'ax:{a.mu.reg_read(UC_X86_REG_AX):x} bx:{a.mu.reg_read(UC_X86_REG_BX):x} '
              f'si:{a.mu.reg_read(UC_X86_REG_SI):x} cf:{a.mu.reg_read(UC_X86_REG_EFLAGS) & 1} '
              f'di:{a.mu.reg_read(UC_X86_REG_DI):x} fs:{(a.mu.reg_read(UC_X86_REG_FS) - LOAD_SEG) & 0xffff:x}')
        after = region()
        diff = ' '.join(f'{lo + i:x}:{b1[i]:x}' for (lo, _), b0, b1 in zip(REGIONS, before, after)
                        for i in changed(b0, b1))
    except Fault:
        rv = f'fault@{a.mu.reg_read(UC_X86_REG_CS) - LOAD_SEG:04X}:{a.mu.reg_read(UC_X86_REG_IP):04X}'
        diff = ''
    w = ' '.join(compact(writes))
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    ev = ':'.join(str(int(x)) for x in env)
    out.write(f'{name} | {w} | {rg} | {ev} | {rv} | {hooks()} | {diff}\n')


def compact(writes):
    """Writes as runs: kind:off:hexbytes for plain bytes (later writes win),
    kind:off:size:value for segment values."""
    plain, segs = {}, []
    for k, o, n, v in writes:
        if k in 'DM':
            segs.append(f'{k}:{o:x}:{n}:{v & ((1 << 8 * n) - 1):x}')
            continue
        for i in range(n):
            plain[(k, (o + i) & 0xffff)] = (v >> (8 * i)) & 0xff
    runs = []
    for (k, o) in sorted(plain):
        if runs and runs[-1][0] == k and runs[-1][1] + len(runs[-1][2]) == o:
            runs[-1][2].append(plain[(k, o)])
        else:
            runs.append([k, o, [plain[(k, o)]]])
    return [f'{k}:{o:x}:{bytes(b).hex()}' for k, o, b in runs] + segs


def changed(b0, b1):
    """Indices where two equal-length buffers differ (blockwise first)."""
    for blk in range(0, len(b0), 256):
        if b0[blk:blk + 256] != b1[blk:blk + 256]:
            for i in range(blk, min(blk + 256, len(b0))):
                if b0[i] != b1[i]:
                    yield i


def near(seg, off):
    return lambda **r: a.call_near(seg, off, **r)


def far(seg, off):
    def run(**r):
        a._setup([0, STUB_SEG], r)
        return a._run(seg, off, STUB_SEG, 0)
    return run


# --- random state -----------------------------------------------------------

def small(lo=-25, hi=25, zero=0.4):
    return 0 if rng.random() < zero else rng.randint(lo, hi)


def record(kind, off, creature=None):
    """A character/creature record with the fields effects touch."""
    if creature is None:
        creature = kind != 'd'
    flags = rng.getrandbits(16)
    flags &= ~(0x1000 | 0x4000 | 0x80 | 0x100)
    if creature:
        flags |= 0x1000
    if rng.random() < 0.25:
        flags |= 0x80
    if rng.random() < (0.6 if kind == 'd' else 0.15):
        flags |= 0x4000
    if rng.random() < 0.1:
        flags |= 0x100
    if rng.random() < 0.5:
        flags &= ~0x7F
    max_hp = rng.randint(1, 200)
    w = [(5, 1, rng.choice(CLASS_IDS) if rng.random() < 0.8 else rng.getrandbits(8)),
         (4, 1, rng.randint(0, 0x18)),
         (6, 1, rng.randint(0, 20)),
         (0x59, 2, rng.choice([0, 1, 2, rng.randint(0, max_hp), max_hp, rng.getrandbits(16)])),
         (0x5B, 2, max_hp),
         (0x5D, 2, rng.choice([rng.randint(0, 20000), rng.getrandbits(16)])),
         (0x66, 2, rng.randint(0, 150)), (0x68, 2, rng.randint(0, 150)),
         (0x40D, 2, flags),
         (0x41C, 2, rng.choice([0, 0, rng.randint(1, 60), rng.getrandbits(16)])),
         (0x41A, 2, rng.getrandbits(16))]
    w += [(0x29 + i, 1, rng.choice([rng.randint(20, 100), rng.randint(0, 255), 0, 1])) for i in range(8)]
    w += [(0x31 + i, 1, rng.randint(0, 255)) for i in range(8)]
    w += [(0x40F + i, 1, rng.randint(0, 3)) for i in range(9)]
    return [(kind, off + f, s, v) for f, s, v in w]


def creature_offset():
    return rng.choice([0, 0x41E, 0x83C])


def target():
    """(kind letter, seg, off) of a record that effects can aim at."""
    if rng.random() < 0.5:
        return 'd', DS, PLAYER
    return 'm', MSEG, creature_offset()


def phase(base, a_phase):
    w = [(base + 0, 2, rng.choice([1, 1, 2, 3, 0, rng.getrandbits(16)])),
         (base + 4, 2, rng.choice([1, 1, 2, 3, 0])),
         (base + 0xE, 2, small(-30, 30) & 0xffff),
         (base + 0x10, 2, small(-30, 30) & 0xffff),
         (base + 0x12, 2, rng.choice([0, 0, rng.randint(-60, 60), rng.getrandbits(16)]) & 0xffff)]
    if a_phase:
        w.append((base + 2, 2, rng.choice([0, 0, 1, 1, 2, 3, rng.getrandbits(16)])))
    else:
        w.append((base + 2, 2, rng.choice([0xFFFF, 0xFFFD, 0xFFFC, 0xFFFB, 0, 1, 2, 3])))
    w += [(base + 6 + i, 1, small(-20, 20) & 0xff) for i in range(8)]
    return w


def slots(targets):
    """All 16 slots: some used (aimed at one of `targets`), some free."""
    w = []
    for i in range(16):
        s = SLOTS + i * SLOT_SIZE
        if rng.random() < 0.45 and targets:
            kind, seg, off = rng.choice(targets)
            f = phase(0, True) + phase(0x14, False)
            f += [(0x28, 2, off)]
            f += [(0x2C + j, 1, rng.randint(0, 255)) for j in range(8)]
            f += [(0x34, 2, rng.getrandbits(16)), (0x36, 2, rng.getrandbits(16)), (0x38, 2, rng.getrandbits(16))]
            f += [(0x3A, 1, rng.choice([0, 1, 4, 6, 9, 10, 11, 12, 0xFF, rng.getrandbits(8)])),
                  (0x3B, 1, rng.randint(0, 0xFE))]
            w += [('d', s + o, n, v) for o, n, v in f]
            w.append(('D', s + 0x2A, 2, seg))
        else:
            if rng.random() < 0.2:
                w += [('d', s + o, 2, rng.getrandbits(16)) for o in range(0, 0x3A, 2) if o != 0x2A]
                # A real segment: arbitrary ones point at different memory
                # in the emulator (relocated) and in the port.
                w.append(('D', s + 0x2A, 2, rng.choice([DS, MSEG, OSEG])))
            w += [('d', s + 0x3A, 1, rng.getrandbits(8)), ('d', s + 0x3B, 1, 0xFF)]
    return w


def creatures():
    w = [('D', 0x1042, 2, MSEG)]
    for off in (0, 0x41E, 0x83C):
        w += record('m', off)
    return w


def env():
    return (rng.random() < 0.3, rng.random() < 0.5, rng.random() < 0.3, rng.random() < 0.3, rng.random() < 0.5)


FLAGS = [0, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x1000]

# --- cases ------------------------------------------------------------------

for _ in range(int(os.environ.get("FX_N", "100"))):
    case('effect_kind', [], near(0x2EBD, 0x01F3), ax=rng.choice([0, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0xFF, 0xFE, rng.getrandbits(8)]))

    # apply_effect: player, creatures, or a third segment, with the creature
    # segment value even (accepted) or odd (refused).
    k, seg, off = rng.choice([('d', DS, PLAYER), ('m', MSEG, creature_offset()), ('o', OSEG, 0x100)])
    cseg = rng.choice([MSEG, MSEG, MSEG + 1])
    w = [('D', 0x1042, 2, cseg)] + record('d', PLAYER) + creatures()[1:] + record('o', 0x100)
    w += slots([('d', DS, PLAYER), ('m', MSEG, 0)]) + [('d', 0x0D39, 1, rng.getrandbits(8)), ('m', 0x0D39, 1, 0), ('o', 0x0D39, 1, 0)]
    if rng.random() < 0.15:  # all slots taken
        w += [('d', SLOTS + i * SLOT_SIZE + 0x3B, 1, rng.randint(0, 0xFE)) for i in range(16)]
    case('apply_effect', w, far(0x2EBD, 0x0102),
         ds=seg, si=off, ax=rng.choice(FLAGS + [rng.getrandbits(16)]), bx=rng.randint(0, 5),
         cx=rng.choice([0, 1, 5, 0x5A0]), dx=rng.randint(0, 5), bp=rng.choice([0xFFFF, 0xFFFD, 0xFFFC, 0xFFFB, 3]))

    ts = [('d', DS, PLAYER), ('m', MSEG, 0), ('m', MSEG, 0x41E)]
    case('end_effect', record('d', PLAYER) + creatures() + slots(ts), near(0x2EBD, 0x00B4),
         si=SLOTS + rng.randint(0, 15) * SLOT_SIZE)

    k, seg, off = rng.choice(ts)
    case('clear_effects_on', record('d', PLAYER) + creatures() + slots(ts), far(0x2EBD, 0x0084), ds=seg, si=off)

    k, seg, off = rng.choice(ts + [('o', OSEG, 0x100)])
    case('find_effect_slot', slots(ts), near(0x2EBD, 0x0190),
         ebx_seg=seg, ebx_off=off, ax=rng.choice([0, 1, 4, 9, 10, rng.getrandbits(8)]))

    w = slots(ts) + [('D', 0x1042, 2, rng.choice([MSEG, OSEG]))]
    w += [('d', SLOTS + i * SLOT_SIZE + 0x28, 2, rng.choice([PLAYER, 0, 0x41E])) for i in range(16)]
    case('fix_slot_segments', w, far(0x2EBD, 0x054C))

    k, seg, off = target()
    case('absorb_damage', record(k, off, creature=rng.random() < 0.3), far(0x29CF, 0x0C4F),
         ds=seg, si=off, ax=rng.choice([rng.randint(-80, 80), rng.getrandbits(16), 0x8000]) & 0xffff)

    # The tick: everything at once.
    case('tick_effects', record('d', PLAYER) + creatures() + slots(ts) + [
            ('d', 0xA836, 2, rng.getrandbits(16)), ('d', 0xA838, 2, rng.getrandbits(16)),
            ('d', 0x0D39, 1, rng.getrandbits(8))],
         far(0x2EBD, 0x0268), env=env())

    k, seg, off = rng.choice(ts + [('o', OSEG, 0x100)])
    case('apply_disease', record('d', PLAYER) + creatures() + record('o', 0x100) + slots(ts) + [
            ('D', 0x1042, 2, rng.choice([MSEG, MSEG + 1]))],
         far(0x2EBD, 0x0573), ds=seg, si=off, ax=rng.randint(10, 25))  # past 25 the table points at relocated words

    p = record('d', PLAYER, creature=False) + [
        ('d', PLAYER + 4, 1, rng.choice([4, 0, 1, 2])), ('d', PLAYER + 5, 1, rng.choice([0x11, 0x0F] + CLASS_IDS)),
        ('d', PLAYER + 0x62, 1, rng.randint(0, 100)), ('d', PLAYER + 0x64, 1, rng.randint(0, 100)),
        ('d', 0x05BE, 2, rng.choice([0, 0, 0x40, rng.getrandbits(16)])), ('d', 0x129A, 2, rng.randint(0, 7))]
    mon = record('m', 0x41E) + [('m', 0x41E + 4, 1, rng.choice([3, 9, 0x16, 5, 10, 11, 12, 13, 14, 0, rng.randint(0, 0x30)]))]
    case('creature_special_attack', creatures() + p + mon + slots(ts), near(0x2AA1, 0x035D),
         env=env(), ds=MSEG, si=0x41E)

    defender = creature_offset()
    case('weapon_enchantments', creatures() + p + slots(ts) + [
            ('d', 0x1402, 2, defender), ('m', defender + 5, 1, rng.choice([0x11] + CLASS_IDS)),
            ('d', 0x0218, 2, rng.choice([rng.randint(0, 2000), 800, 799, rng.getrandbits(16)]))],
         near(0x2AA1, 0x08DC), env=env(), ax=rng.randint(0, 200))

    case('vampiric_heal', p, near(0x2AA1, 0x0956), env=env(), ax=rng.choice([rng.randint(0, 200), rng.getrandbits(16)]))

    w = record('d', PLAYER) + creatures() + [
        ('d', 0x05B1, 2, rng.getrandbits(16)), ('d', 0x0FF9, 2, rng.choice([0, 1, 2, 3]))]
    w += [(kk, o + 0x29 + rng.randint(0, 7), 1, 0) for kk, o in [('d', PLAYER), ('m', 0), ('m', 0x41E), ('m', 0x83C)] if rng.random() < 0.4]
    case('drain_death_check', w, far(0x2686, 0x0004), env=env())

    w = slots(ts) + record('d', PLAYER) + [('d', 0x05B1, 2, rng.getrandbits(16))]
    w += [('d', SLOTS + i * SLOT_SIZE + 0x3A, 1, rng.choice([4, 4, 1, 0xFF])) for i in range(16)]
    case('rebase_drain_snapshots', w, far(0x2EBD, 0x0617), ax=rng.randint(0, 300), bx=rng.randint(0, 7))
    case('attr_drained', w, far(0x2EBD, 0x0657), bp=rng.randint(0, 7))

# Special attacks aimed at the paralysing and disease-carrying types, with
# the chance and the player's TIER_B often raised so the whole path runs.
for _ in range(int(os.environ.get("FX_SPECIAL", "150"))):
    ty = rng.choice([1, 3, 9, 10, 13, 22, 12, 7])
    p = record('d', PLAYER, creature=False) + [
        ('d', PLAYER + 4, 1, rng.choice([0, 1, 2, 3, 5, 4])),
        ('d', PLAYER + 5, 1, rng.choice(CLASS_IDS)),
        ('d', PLAYER + 0x62, 1, rng.choice([rng.randint(100, 255), rng.randint(0, 100)])),
        ('d', 0x05BE, 2, rng.choice([0, 0, 0, 0x40])), ('d', 0x129A, 2, rng.randint(0, 7))]
    if rng.random() < 0.5 and ty != 0x16:
        p.append(('d', 0x2DE6 + ty - 1, 1, rng.choice([100, 99, 50])))
    mon = record('m', 0x41E) + [('m', 0x41E + 4, 1, ty)]
    case('creature_special_attack', creatures() + p + mon + slots([('d', DS, PLAYER), ('m', MSEG, 0)]),
         near(0x2AA1, 0x035D), env=env(), ds=MSEG, si=0x41E)

# Ticks that land exactly on their bounds (HP, SP and attributes reach
# the maximum or 255 with nothing to spare), one slot on the player.
def to256(v):
    q, r = divmod(abs(v) * 256, 100)
    q += r >= 50
    return q if v >= 0 else -q

for _ in range(int(os.environ.get("FX_EXACT", "100"))):
    s = SLOTS + rng.randint(0, 15) * SLOT_SIZE
    p = record('d', PLAYER, creature=False)
    max_hp, max_sp = rng.randint(30, 200), rng.randint(30, 150)
    d_hp, d_sp = rng.randint(1, 20), rng.randint(1, 20)
    exact = rng.random() < 0.7
    p += [('d', PLAYER + 0x5B, 2, max_hp), ('d', PLAYER + 0x59, 2, max_hp - d_hp - (0 if exact else rng.randint(-1, 1))),
          ('d', PLAYER + 0x68, 2, max_sp), ('d', PLAYER + 0x66, 2, max_sp - d_sp - (0 if exact else 1)),
          ('d', PLAYER + 0x40D, 2, rng.getrandbits(16) & ~0x10C0 | 0x4000), ('d', PLAYER + 0x41C, 2, 0),
          ('d', PLAYER + 0x38, 1, 0xCC)]
    deltas = [rng.choice([0, rng.randint(1, 30)]) for _ in range(8)]
    p += [('d', PLAYER + 0x29 + i, 1, 0xFF - to256(dv) if dv and exact else rng.randint(0, 0xFF - to256(dv)))
          for i, dv in enumerate(deltas)]
    w = slots([]) + p
    # Phase B (A has run out), the only phase whose end depends on the
    # bounds; attribute bounds are then the snapshot.
    w += [('d', s + 0x2C + i, 1, 0xFF if exact else rng.randint(0, 0xFF)) for i in range(8)]
    w += [('d', s + 0, 2, 1), ('d', s + 2, 2, 0), ('d', s + 4, 2, 1),
          ('d', s + 0x22, 2, d_hp), ('d', s + 0x24, 2, d_sp), ('d', s + 0x26, 2, 0),
          ('d', s + 0x16, 2, rng.choice([0xFFFF, 2, 0xFFFC])), ('d', s + 0x14, 2, 1), ('d', s + 0x18, 2, 1),
          ('d', s + 0x28, 2, PLAYER), ('D', s + 0x2A, 2, DS), ('d', s + 0x3A, 1, rng.choice([0, 4, 6])), ('d', s + 0x3B, 1, 0)]
    w += [('d', s + 0x1A + i, 1, dv) for i, dv in enumerate(deltas)]
    case('tick_effects', w + [('d', 0x0D39, 1, 0)], far(0x2EBD, 0x0268), env=env())


# Timed spell flags (+41A) with their far table at OSEG:0200, and the
# spell resistance check.
TABLE = 0x0200

def flag_table():
    w = [('d', 0x1020, 2, TABLE), ('D', 0x1022, 2, OSEG)]
    w += [('o', TABLE + 2 * i, 2, rng.choice([0, 1, 1, 2, 3, rng.randint(0, 120), rng.getrandbits(16)]))
          for i in range(0, 0x240 // 2)]
    for k in range(8):
        e = 0x0C57 + k * 0x1C
        w += [('d', e + 0x0F, 2, rng.getrandbits(16)), ('d', e + 0x12, 2, rng.choice([0, 0x41E, 0x83C, 0x100]))]
    return w

def spell_flags(kind, off):
    v = rng.choice([0, rng.getrandbits(16), 0x1000, 0x400, 0x800, 0x1C00, 0x1C7C & rng.getrandbits(16)])
    return [(kind, off + 0x41A, 2, v)]

for _ in range(int(os.environ.get("FX_SPELL", "100"))):
    w = creatures() + record('d', PLAYER) + flag_table() + spell_flags('d', PLAYER)
    w += [x for o in (0, 0x41E, 0x83C) for x in spell_flags('m', o)]
    case('tick_spell_flags', w + [('d', 0x0D39, 1, rng.getrandbits(8))], far(0x2CDD, 0x0964))

    k, seg, off = rng.choice([('d', DS, PLAYER), ('m', MSEG, 0), ('m', MSEG, 0x41E), ('m', MSEG, 0x83C), ('m', MSEG, 0x200)])
    case('spell_flag_table', creatures() + flag_table(), near(0x2CDD, 0x09E0), fs=seg, dx=off)

    k, seg, off = rng.choice([('d', DS, PLAYER), ('m', MSEG, 0), ('m', MSEG, 0x41E), ('m', MSEG, 0x83C)])
    w = creatures() + record('d', PLAYER) + flag_table() + spell_flags(k, off)
    w += [(k, off + 5, 1, rng.choice([0x23, 0x23] + CLASS_IDS))]
    caster = rng.choice([PLAYER, 0, 0x41E, 0x83C, 0x1F00])
    w += [('d', 0x1292, 2, rng.choice([0, 1, 2, 5, 10, 21, 150, 300, 0xFFF0])), ('d', 0xA886, 2, 0xB000),
          ('d', 0xB004, 2, caster), ('d', 0xB006, 2, rng.getrandbits(16)),
          ('d', 0x0D5A, 2, rng.randint(0, 15)), ('d', 0x1276, 2, rng.randint(0, 5)),
          ('o', 0x100 + 0x32, 2, rng.choice([rng.randint(0, 200), rng.getrandbits(16)]))]
    case('spell_stopped', w, far(0x2CDD, 0x0A38), env=env(), fs=seg, edx=off, gs=OSEG, si=0x100)

    # Sorcerers at the absorb cap: the first random byte is chosen around
    # C0h by picking the seed, and (INT + WIL) / 2 straddles C0h.
    want = rng.choice([0xBF, 0xC0, 0xC1])
    while True:
        seed = rng.getrandbits(32) | 1
        if ((seed * 0x006D002D) & 0xFFFFFFFF) >> 16 & 0xFF == want:
            break
    s_int = rng.randint(0xB8, 0xFF)
    w = creatures() + record('d', PLAYER) + flag_table() + [
        ('d', PLAYER + 5, 1, 0x23), ('d', PLAYER + 0x2A, 1, s_int),
        ('d', PLAYER + 0x2B, 1, rng.randint(max(0, 0x17E - s_int), 0xFF)),
        ('d', 0x1292, 2, 3), ('o', 0x100 + 0x32, 2, rng.randint(1, 200)), ('d', 0x91F6, 4, seed)]
    case('spell_stopped', w, far(0x2CDD, 0x0A38), fs=DS, edx=PLAYER, gs=OSEG, si=0x100)

    # Resist-magic rolls at their exact boundary: the seed fixes the
    # percent roll p, and the table strength puts the chance at p or p - 1;
    # or a level gap past 8 bits, with strengths where that matters.
    p_want = rng.randint(0, 99)
    while True:
        seed = rng.getrandbits(32) | 1
        if (((seed * 0x006D002D) & 0xFFFFFFFF) >> 16) % 100 == p_want:
            break
    level = rng.randint(0, 30)
    if rng.random() < 0.5:
        caster, strength = level + 1, p_want - rng.choice([0, 1])
    else:
        gap = rng.choice([-144, -130, 130, 150])        # (level + 1) - caster
        caster = (level + 1 - gap) & 0xFFFF
        bonus8 = ((gap & 0xFF) ^ 0x80) - 0x80
        strength = p_want - bonus8 * 5 + rng.choice([0, -1])
    w = creatures() + record('d', PLAYER) + flag_table() + [
        ('d', PLAYER + 5, 1, rng.choice(CLASS_IDS[:3] + CLASS_IDS[4:])), ('d', PLAYER + 6, 1, level),
        ('d', PLAYER + 0x41A, 2, 0x1000), ('d', 0x1292, 2, caster),
        ('o', TABLE + 0x30 + 2, 2, strength & 0xFFFF),
        ('o', 0x100 + 0x32, 2, rng.randint(1, 200)), ('d', 0x91F6, 4, seed)]
    case('spell_stopped', w, far(0x2CDD, 0x0A38), fs=DS, edx=PLAYER, gs=OSEG, si=0x100)
