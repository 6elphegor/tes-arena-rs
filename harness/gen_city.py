"""Emit test vectors for the city generator (src/city.rs).

Each line: name | writes | regs | result | memory diff [| blocks]
  writes: off:size:value (DGROUP, applied before the call)
  result: "fault", "hang", or for block names "load:<offset>" / "none"
  diff:   linear:new for every changed byte from 3276:0000 to the end of
          DGROUP (image-relative)
  blocks: for generate_city, "pos=name" per block in load order

Stubbed (patched to return before anything runs, since unicorn caches
translated code): 0000:5C86 and 0000:E87C (screen/loader calls), 1194:05A9,
and the block file load at 1194:03FA, which jumps straight to the RET at
0487. A hook at 03FA records each block's name first.
"""
import random, sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, LOAD_SEG, STUB_SEG
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *

a = Arena(sys.argv[1])
rng = random.Random(1994_11)
out = open(sys.argv[2], 'w')
S = 0x1194
LO, HI = 0x3276 * 16, 0x3BA0 * 16 + 0x10000  # image-relative
PLAN = 0x486B
LOAD_AT = a.lin(LOAD_SEG + S, 0x03FA)


def patch(seg, off, code):
    a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)


patch(0, 0x5C86, b'\xcb')                 # retf
patch(0, 0xE87C, b'\xcb')                 # retf
patch(S, 0x05A9, b'\xc3')                 # ret
patch(S, 0x03FA, b'\xe9' + struct.pack('<h', 0x0487 - (0x03FA + 3)))  # jmp 0487

stop_at_load = False
blocks = []


def on_load(mu, addr, size, _):
    si = mu.reg_read(UC_X86_REG_SI)
    name = bytes(mu.mem_read(a.lin(a.ds, si), 16)).split(b'\0')[0]
    blocks.append((a.r16(0xA888), name))
    if stop_at_load:
        mu.emu_stop()


a.mu.hook_add(UC_HOOK_CODE, on_load, begin=LOAD_AT, end=LOAD_AT)


def divides_by_zero(cs, ip):
    """Is CS:IP a DIV/IDIV whose divisor is 0? (Unicorn can stop on a
    faulting DIV silently; a hang can also stop on a good one.)"""
    mu = a.mu
    op, modrm = mu.mem_read(a.lin(cs, ip), 2)
    if op not in (0xF6, 0xF7) or (modrm >> 3) & 7 not in (6, 7):
        return False
    mod, rm = modrm >> 6, modrm & 7
    if mod == 3:
        regs = [UC_X86_REG_AX, UC_X86_REG_CX, UC_X86_REG_DX, UC_X86_REG_BX,
                UC_X86_REG_SP, UC_X86_REG_BP, UC_X86_REG_SI, UC_X86_REG_DI]
        v = mu.reg_read(regs[rm])
    elif mod == 2 and rm == 4:  # [si + disp16]
        disp = struct.unpack('<h', mu.mem_read(a.lin(cs, ip + 2), 2))[0]
        v = a.r16((mu.reg_read(UC_X86_REG_SI) + disp) & 0xffff)
    else:
        return True
    return (v & (0xff if op == 0xF6 else 0xffff)) == 0


def _stop(mu, addr, size, _):
    mu.emu_stop()


def run(off, far=False, stop=None, **regs):
    """Call 1194:off, or run it until 1194:stop. Returns 'ret', 'load',
    'fault' or 'hang'."""
    mu = a.mu
    if far:
        a._setup([0, STUB_SEG], regs)
        until = a.lin(STUB_SEG, 0)
    else:
        a._setup([0xFFF0], regs)
        until = a.lin(LOAD_SEG + S, 0xFFF0)
    mu.reg_write(UC_X86_REG_CS, LOAD_SEG + S)
    mu.reg_write(UC_X86_REG_IP, off)
    if stop is not None:
        until = a.lin(LOAD_SEG + S, stop)
        h = mu.hook_add(UC_HOOK_CODE, _stop, begin=until, end=until)
    a.fault = None
    mu.emu_start(a.lin(LOAD_SEG + S, off), until, count=3_000_000)
    if stop is not None:
        mu.hook_del(h)
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if a.fault is not None:
        return 'fault'
    if a.lin(cs, ip) == until:
        return 'ret'
    if a.lin(cs, ip) == LOAD_AT and stop_at_load:
        return 'load'
    return 'fault' if divides_by_zero(cs, ip) else 'hang'


base = None


def case(name, writes, off, far=False, stop=None, **regs):
    global base, stop_at_load
    writes = [(0x91F0, 2, 0), (0x91F6, 4, rng.getrandbits(32))] + writes
    region = lambda: bytes(a.mu.mem_read(LOAD_SEG * 16 + LO, HI - LO))
    if base is None:
        base = region()
    a.mu.mem_write(LOAD_SEG * 16 + LO, base)
    for o, size, v in writes:
        a.mu.mem_write(a.lin(a.ds, o), (v & ((1 << 8 * size) - 1)).to_bytes(size, 'little'))
    before = region()
    blocks.clear()
    stop_at_load = name == 'block_name'
    r = run(off, far, stop, **regs)
    after = region()
    if r in ('fault', 'hang'):
        result, diff = r, ''
    else:
        if name == 'block_name':
            result = f'load:{a.mu.reg_read(UC_X86_REG_SI):x}' if r == 'load' else 'none'
        else:
            result = 'ok'
        diff = ' '.join(f'{LO + i:x}:{y:x}' for i, (x, y) in enumerate(zip(before, after)) if x != y)
    w = ' '.join(f'{o:x}:{s}:{v & ((1 << 8 * s) - 1):x}' for o, s, v in writes)
    rg = ' '.join(f'{k}:{v:x}' for k, v in regs.items())
    line = f'{name} | {w} | {rg} | {result} | {diff}'
    if name == 'generate_city' and result == 'ok':
        line += ' | ' + ' '.join(f'{p:x}={n.decode()}' for p, n in blocks)
    out.write(line + '\n')


def cell():
    r = rng.random()
    if r < 0.6:
        return 0
    if r < 0.8:
        return 1
    if r < 0.9:
        return rng.randint(2, 8)
    return rng.getrandbits(8)


def plan_writes(n):
    return [(PLAN + i, 1, cell()) for i in range(n)]


def dim():
    return rng.choice([4, 5, 6, 6, 5, 4, 3, 2, 7, 8, 9, 1, 0])


for _ in range(400):
    d = dim()
    full = rng.random() < 0.05
    plan = [(PLAN + i, 1, rng.choice([1, 2, 7])) for i in range(d * d)] if full else plan_writes(d * d)
    case('place_block', [(0xA88C, 2, d * d)] + plan, 0x04BE, ax=rng.randint(0, 6))
for _ in range(400):
    d = dim()
    case('city_plan', [(0xA88C, 2, d * d)] + plan_writes(d * d), 0x0316, stop=0x036A)
for _ in range(600):
    c = rng.choice([rng.randint(2, 8), rng.randint(2, 8), rng.randint(0, 1), rng.getrandbits(8)])
    case('block_name', [], 0x03A2, si=c)
for _ in range(500):
    d = dim()
    w = [(0xA46, 1, 8 if rng.random() < 0.1 else rng.randint(0, 7)), (0xA43, 1, d),
         (0xA44, 2, rng.getrandbits(16)), (0xA48, 2, rng.getrandbits(16)), (0xA4A, 2, rng.getrandbits(16)),
         (0x9030, 2, rng.getrandbits(16)), (0xC398, 2, rng.getrandbits(16))]
    case('generate_city', w + plan_writes(d * d), 0x0292, far=True)
