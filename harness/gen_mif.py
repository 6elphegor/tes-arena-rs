"""Emit test vectors for src/bsa.rs and src/mif.rs from the original code.

Usage: gen_mif.py ACD_unpacked.EXE GAME_DIR OUT

Runs the original under a minimal DOS file layer (harness/dos.py) backed
by the user's game directory and with the game's heap set up (the
decompressor and block loader allocate from it), so the file code (0000:67C4.., the BSA
directory scan) runs unmodified. No game data goes into the vectors:
real inputs are named (BSA file names, chunk indices) and outputs are
FNV-1a digests.

Lines (fields separated by " | "):
  open  | count dir                       BSA open (0000:6F3A)
  find  | name_hex | cf off size enc      directory scan (0000:692E)
  read  | name | ops | results | digest   open + reads/seeks (67C4, 6BEC, 6D32)
  lzhuf | src | ax | digest | globals     decoder (3065:0D2D)
  copy  | kind pos seed | digest          plane copies (1194:04DD, 0542, 051A)
  block | name pos | digests             block load (1194:03FA..0487)
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_0B5A)
out = open(OUT, 'w', buffering=1)

FLOOR_SEG, MAP1_SEG, MAP2_SEG, DECODE_SEG = 0x7000, 0x7800, 0x8000, 0x8800
SRC_SEG = 0x5C00
PLANES = (FLOOR_SEG, MAP1_SEG, MAP2_SEG, DECODE_SEG)


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


def fill(i):
    """Deterministic plane filler shared with the Rust test."""
    return (i * 7 + 3) & 0xff


PLANE_FILL = bytes(fill(i) for i in range(0x10000))

a = DosArena(EXE, GAME)
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)
out.write(f'open | {a.r16(0xA926):x} {a.r16(0xA922) | a.r16(0xA924) << 16:x}\n')

# Plane segments the loader writes through.
for var, seg in ((0xCCD0, FLOOR_SEG), (0xD806, MAP1_SEG), (0xD748, MAP2_SEG), (0x9030, DECODE_SEG)):
    a.w16(var, seg)
BASE = a.snapshot()

bsa = open(os.path.join(GAME, a.names['GLOBAL.BSA']), 'rb').read()
count = struct.unpack_from('<H', bsa, 0)[0]
d = len(bsa) - 18 * count
NAMES = [bsa[d + 18 * i: d + 18 * i + 14].split(b'\0')[0] for i in range(count)]
SIZES = [struct.unpack_from('<I', bsa, d + 18 * i + 14)[0] for i in range(count)]
OFFS = [2 + sum(SIZES[:i]) for i in range(count)]


def fresh():
    a.restore(BASE)
    for s in PLANES:
        a.mu.mem_write(s * 16, PLANE_FILL)


def plane_digests():
    return ' '.join(f'{fnv(a.seg_bytes(s)):x}' for s in PLANES)


def put_name(off, name):
    a.mu.mem_write(a.lin(a.ds, off), name + b'\0')


def cf():
    from unicorn.x86_const import UC_X86_REG_EFLAGS
    return a.mu.reg_read(UC_X86_REG_EFLAGS) & 1


# --- find ------------------------------------------------------------------

def case_variant(n):
    return bytes(c ^ 0x20 if 0x41 <= (c & 0xdf) <= 0x5a and rng.random() < 0.5 else c for c in n)


find_names = [case_variant(NAMES[i]) for i in rng.sample(range(count), 250)]
find_names += [NAMES[0], NAMES[-1], b'nosuch.mif', b'tvbd1a.mi', b'TVBD1A.MIFX', b'',
               b'x', b'a.inf', b'abcdefghijklmnopqrst']
find_names += [n for n in NAMES if n.upper().endswith((b'.INF', b'.TXT'))][:10]
for name in find_names:
    fresh()
    put_name(0x370C, name)
    a.call_near(0, 0x692E, dx=0x370C)
    c = cf()
    out.write(f'find | {name.hex()} | {c:x} {a.r32(0x9198):x} {a.r32(0x9184):x} {a.mu.mem_read(a.lin(a.ds, 0x9190), 1)[0]:x}\n')


# --- read: open + reads/seeks through the file layer ------------------------

BUF = 0xA000  # DGROUP scratch for reads


def file_ops(name, ops):
    """ops: ('r', n) or ('s', whence, off). Returns per-op results."""
    fresh()
    put_name(0x370C, name)
    a.call_far(0, 0x67C4, si=0x370C)
    res = [f'o{cf()}']
    data = b''
    for op in ops:
        if op[0] == 'r':
            a.mu.mem_write(a.lin(a.ds, BUF), bytes(range(256)) * (op[1] // 256 + 1))
            ax = a.call_far(0, 0x6BEC, si=BUF, cx=op[1])
            data += bytes(a.mu.mem_read(a.lin(a.ds, BUF), op[1]))
            res.append(f'{ax:x}')
        else:
            r = a.call_far(0, 0x6D32, ax=op[2] & 0xffff, dx=op[2] >> 16, cx=op[1])
            res.append(f'{r:x}')
    a.call_far(0, 0x6742)
    return res, data


def ops_text(ops):
    return ','.join(f'r{o[1]:x}' if o[0] == 'r' else f's{o[1]}:{o[2]:x}' for o in ops)


encrypted = [n for n in NAMES if n.upper().endswith((b'.INF', b'.TXT'))]
read_names = rng.sample(encrypted, 12) + rng.sample([n for n in NAMES if n.upper().endswith(b'.MIF')], 8)
for name in read_names:
    i = NAMES.index(name)
    size = SIZES[i]
    for _ in range(3):
        ops = []
        for _ in range(rng.randint(1, 5)):
            if rng.random() < 0.7:
                ops.append(('r', rng.choice([1, 7, 64, 300, rng.randint(1, 2000)])))
            else:
                ops.append(('s', rng.choice([0, 1, 2]),
                            rng.choice([0, rng.randint(0, max(size, 1)), size + rng.randint(0, 40),
                                        (-rng.randint(1, 50)) & 0xffffffff])))
        res, data = file_ops(name, ops)
        out.write(f'read | {name.decode()} | {ops_text(ops)} | {" ".join(res)} | {fnv(data):x}\n')


# --- lzhuf ------------------------------------------------------------------

def chunks(data):
    """(tag, payload) of the FLOR/MAP1/MAP2 chunks, walked as 03FA does."""
    p, res = 6 + 0x3D + 6, []
    while p + 6 <= len(data):
        tag, n = data[p:p + 4], struct.unpack_from('<H', data, p + 4)[0]
        p += 6
        if tag in (b'FLOR', b'MAP1', b'MAP2'):
            res.append((tag, data[p:p + n]))
        p += n
    return res


def lzhuf(src):
    fresh()
    a.mu.mem_write(SRC_SEG * 16, src)
    a._setup([0, STUB_SEG, 0, SRC_SEG, 0, DECODE_SEG, len(src)], {})
    r = a._run(0x3065, 0x0D2D, STUB_SEG, 0)
    g = [a.mu.mem_read(a.lin(a.ds, 0xB53E), 1)[0], a.mu.mem_read(a.lin(a.ds, 0xB53F), 1)[0]]
    g += [a.r16(o) for o in (0xB540, 0xB542, 0xEEE6, 0xEEE8, 0xEECA, 0xEECC, 0xEECE, 0xEED0, 0xEED2, 0xEED4)]
    return r['ax'], fnv(a.seg_bytes(DECODE_SEG)), ' '.join(f'{x:x}' for x in g)


mifs = [n for n in NAMES if n.upper().endswith(b'.MIF')]
for name in rng.sample(mifs, 40):
    i = NAMES.index(name)
    data = bsa[OFFS[i]:OFFS[i] + SIZES[i]]
    for k, (tag, src) in enumerate(chunks(data)):
        if k > 2:
            break
        ax, h, g = lzhuf(src)
        out.write(f'lzhuf | bsa:{name.decode()}:{k} | {ax:x} | {h:x} | {g}\n')
        # Truncated input: reads past the end take the EOF paths.
        if len(src) > 8 and rng.random() < 0.5:
            cut = src[:rng.randint(2, len(src) - 1)]
            ax, h, g = lzhuf(cut)
            out.write(f'lzhuf | cut:{name.decode()}:{k}:{len(cut):x} | {ax:x} | {h:x} | {g}\n')
for _ in range(60):
    n = rng.randint(2, 300)
    src = bytes(rng.getrandbits(8) for _ in range(n))
    if rng.random() < 0.7:
        src = struct.pack('<H', rng.randint(0, 2000)) + src[2:]
    ax, h, g = lzhuf(src)
    out.write(f'lzhuf | hex:{src.hex()} | {ax:x} | {h:x} | {g}\n')


# --- plane copies -----------------------------------------------------------

def copy_words(seed):
    """400 source words from a 32-bit LCG (mirrored in tests/mif.rs);
    floor values 0C00/0D00/0E00 are frequent so the remap is exercised."""
    words, x = [], seed
    for _ in range(400):
        x = (x * 1103515245 + 12345) & 0xffffffff
        w = x >> 16
        words.append((0xC00, 0xE00, 0xD00, (x >> 8) & 0xffff)[w & 3])
    return words


for _ in range(90):
    kind = rng.choice([(0x04DD, 'flor'), (0x0542, 'map1'), (0x051A, 'map2')])
    pos = rng.choice([rng.randint(0, 0xffff), rng.randint(0, 5) * 0x28 + rng.randint(0, 5) * 0x1400])
    seed = rng.getrandbits(32)
    fresh()
    words = copy_words(seed)
    a.mu.mem_write(DECODE_SEG * 16, struct.pack('<400H', *words))
    a.w16(0xA888, pos)
    a.call_near(0x1194, kind[0])
    out.write(f'copy | {kind[1]} {pos:x} {seed:x} | {plane_digests()}\n')


# --- wall post-pass (1194:05A9) ---------------------------------------------

def wall_words(seed):
    """0x4000 wall-plane words from the copy LCG (mirrored in tests/mif.rs),
    biased to the 8xxx and 9x34/9x35 words the pass rewrites."""
    words, x = [], seed
    for _ in range(0x4000):
        x = (x * 1103515245 + 12345) & 0xffffffff
        t, v = (x >> 16) & 7, (x >> 4) & 0xffff
        if t < 2:
            words.append(0x8000 | (v & 0x0f00) | (v & 0x1f))
        elif t < 4:
            words.append(0x9000 | (v & 0x0f00) | (0x34 + (v & 1)))
        elif t < 5:
            words.append(0x9000 | (v & 0x0fff))
        else:
            words.append(v)
    return words


# The pass runs on its own machine (whole cities are in gen_level.py).
wa = DosArena(EXE, GAME)
for var, seg in ((0xCCD0, FLOOR_SEG), (0xD806, MAP1_SEG), (0xD748, MAP2_SEG), (0x9030, DECODE_SEG)):
    wa.w16(var, seg)
WALL_BASE = wa.snapshot()
for _ in range(40):
    wa.restore(WALL_BASE)
    for s in PLANES:
        wa.mu.mem_write(s * 16, PLANE_FILL)
    seed = rng.getrandbits(32)
    wa.mu.mem_write(MAP1_SEG * 16, struct.pack('<16384H', *wall_words(seed)))
    variant = rng.choice([2, 0x82, 0, 1, 3])
    w = rng.choice([0, 1, 2, 64, 128, rng.randint(1, 300)])
    h = rng.choice([0, 1, 64, 128, rng.randint(1, 300)])
    types = [rng.randint(0, 0x1f) for _ in range(5)]
    # 05A9 reads these with DS = the wall plane, so they go there.
    wa.mu.mem_write(wa.lin(MAP1_SEG, 0xBB09), bytes([variant]))
    wa.w16(0xC357, w)
    wa.w16(0xC359, h)
    for o, t in zip((0xC945, 0xC946, 0xC947, 0xC948, 0xC951), types):
        wa.mu.mem_write(wa.lin(MAP1_SEG, o), bytes([t]))
    wa.call_near(0x1194, 0x05A9)
    digests = ' '.join(f'{fnv(wa.seg_bytes(s)):x}' for s in PLANES)
    out.write(f'wall | {seed:x} {variant:x} {w:x} {h:x} {" ".join(f"{t:x}" for t in types)} | {digests}\n')


# --- block loads --------------------------------------------------------------

blocks = [n for n in mifs if len(n) >= 6 and n[2:4].upper() == b'BD']
others = [n for n in mifs if n not in blocks]
for name in rng.sample(blocks, 60) + rng.sample(others, 15):
    fresh()
    pos = rng.randint(0, 5) * 0x28 + rng.randint(0, 5) * 0x1400
    a.w16(0xA888, pos)
    lname = name.lower()
    put_name(0x370C, lname)
    try:
        a.run_range(0x1194, 0x03FA, 0x0487, si=0x370C)
        res = plane_digests()
    except Fault:
        res = 'fault'
    out.write(f'block | {lname.decode()} {pos:x} | {res}\n')

