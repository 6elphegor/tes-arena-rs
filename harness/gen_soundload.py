"""Emit test vectors for the sound loader (23BA:0217, src/sound.rs
`sound_load`) from the original code.

Usage: gen_soundload.py ACD_unpacked.EXE GAME_DIR OUT   (SHARD=k/K: par.py)

Runs under the DOS file layer (harness/dos.py) with EMS on (32 banks of
64 KB through the frame at 8800h) and the heap and INF buffer block of
gen_info.py. The sound driver's call 85h (0FFD:0D4C, the sample readied)
is a RETF: it is the front end's.

Lines (fields separated by " | "):
  load | on banks ems buffer kept fill table names | digests
      23BA:0217 called with SI at 929E holding `names` (comma-separated;
      `on` 8F64, `banks` C8FC/C8FD as 4 hex digits, `ems` 8FA6, `buffer`
      the driver's buffer EB89 (offset; segment 7000h), `kept` the 8
      bytes at 45F0 and 1044 (hex, 16 bytes), `fill` the slots' bytes
      before (44F0 + i: i * fill + (fill >> 3)), `table` the 32 bank free
      counts at 8FAA (hex words, 4 digits each).
  fight | ax ems | digests
      23BA:01C7 called with AX `ax` (the creature's sound, 100D) and
      `ems` 8FA6: the AX-th name at 43D7 loaded into bank 15h at AFC8h,
      the file slot at 45F0 made.
  inf | mif banks | digests
      as gen_info.py's info cases (flags 400h, section 80h, the level's
      own INF), with sound on, the hook CD4A at 23BA:0217 and the banks
      C8FC/C8FD `banks`: the level's sounds loaded by the INF loader.
Digests: FNV-1a of the changed DGROUP bytes (offset, value; the open
name's segment at A90A image-relative), of the 32 banks as the program
sees them, and of the INF buffer block. "fault" for a CPU fault, "hang"
where the budget runs out.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG, fnv, mine

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_0217)
out = open(OUT, 'w', buffering=1)

FRAME, BUF_SEG, NAME, INFO, LIST = 0x8800, 0x7000, 0x0A36, 0xE2BE, 0x929E
BLOCK = 0x3E80

a = DosArena(EXE, GAME)
a.std_devices = True
a.patch(0x0FFD, 0x0D4C, b'\xcb')  # the driver's 85h
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
a.enable_ems(FRAME)
r = a.call_near(0, 0x51EC, eax=BLOCK)
a.w16(0xBA54, r['ax'])
a.w16(0xBA56, r['dx'])
a.w16(0xCB8A, FRAME)
a.w16(0xEB8B, BUF_SEG)
a.w16(0xEB91, 1)
for hook in (0xA904, 0xA90E, 0x8F70):
    assert a.r32(hook) == 0, hex(hook)
BASE = a.snapshot()

bsa = open(a.names['GLOBAL.BSA'], 'rb').read()
count = struct.unpack_from('<H', bsa, 0)[0]
d = len(bsa) - 18 * count
ENTRIES = {}
o = 2
for i in range(count):
    name = bsa[d + 18 * i: d + 18 * i + 14].split(b'\0')[0].upper()
    size = struct.unpack_from('<I', bsa, d + 18 * i + 14)[0]
    ENTRIES[name] = (o, size)
    o += size
VOCS = sorted(n for n, (_, s) in ENTRIES.items() if n.endswith(b'.VOC') and s < 0xF000)
SMALL = [n for n in VOCS if ENTRIES[n][1] < 0x2000]
MIFS = sorted({n for n in ENTRIES if n.endswith(b'.MIF')}
              | {n.upper().encode() for n in os.listdir(GAME) if n.upper().endswith('.MIF')})


def has_info(name):
    """Whether the level file's first level has an INFO chunk (its INF,
    encrypted in the BSA, is taken to have a @SOUND section: most do)."""
    path = a.names.get(name.decode())
    if path is not None:
        data = open(path, 'rb').read()
    elif name in ENTRIES:
        o, n = ENTRIES[name]
        data = bsa[o:o + n]
    else:
        return False
    p = 6 + struct.unpack_from('<H', data, 4)[0] + 6
    while p + 6 <= len(data) and data[p:p + 4] != b'LEVL':
        if data[p:p + 4] == b'INFO':
            return True
        p += 6 + struct.unpack_from('<H', data, p + 4)[0]
    return False


INFO_MIFS = [n for n in MIFS if has_info(n)]


def unrelocate(b):
    b = bytearray(b)
    if struct.unpack_from('<H', b, 0xA90A)[0] == a.ds:
        struct.pack_into('<H', b, 0xA90A, a.ds - LOAD_SEG)
    return b


def digests(before):
    before, after = unrelocate(before), unrelocate(a.seg_bytes(a.ds))
    changed = bytearray()
    for i in range(0x10000):
        if after[i] != before[i]:
            changed += struct.pack('<HB', i, after[i])
    banks = b''.join(a.ems_bank(b) for b in range(0x20))
    seg, off = a.r16(0xBA56), a.r16(0xBA54)
    block = bytes(a.mu.mem_read(a.lin(seg, off), BLOCK))
    return f'{fnv(changed):x} {fnv(banks):x} {fnv(block):x}'


def slots(fill):
    """The 200h bytes the slots hold before: byte i (i * fill + (fill >> 3))."""
    return bytes((i * fill + (fill >> 3)) & 0xFF for i in range(0x200))


def free_count():
    return rng.choice([0, 0xFFFF, 0xFFFF, rng.getrandbits(16), rng.randint(0, 0x3000)])


LOADS, INFS, FIGHTS = 1600, 200, 300
total = LOADS + INFS + FIGHTS
for n in range(LOADS):
    on = rng.choice([1] * 12 + [0, rng.getrandbits(16) | 1])
    first = rng.choice([rng.randint(0, 0x1F), rng.randint(0x10, 0x16), rng.randint(0, 0x40)])
    k = rng.choice([1, 2, 3, 4, rng.randint(1, 8), rng.randint(1, 0x20)])
    k = min(k, 0x41 - first)  # past 902Bh the table's writes reach the EMS driver's state
    ems = rng.choice([1] * 10 + [0])
    buf = rng.choice([0, 0, rng.randint(0, 0x80) * 4, rng.randint(0, 0x200)])
    kept = bytes(rng.getrandbits(8) for _ in range(16))
    fill = rng.choice([0, rng.getrandbits(16)])
    table = [free_count() for _ in range(0x20)]
    shape = rng.random()
    if shape < 0.05:
        names = [rng.choice(SMALL) for _ in range(rng.randint(60, 72))]
    else:
        names = [rng.choice(VOCS) for _ in range(rng.randint(0, 14))]
    if rng.random() < 0.2 and names:
        names[rng.randrange(len(names))] = rng.choice([b'NOSUCH.VOC', b'MISSING%d.VOC' % rng.randint(0, 9)])
    names = [x.lower() if rng.random() < 0.5 else x for x in names]
    if not mine(n, total):
        continue
    a.restore(BASE)
    a.w16(0x8F64, on)
    a.mu.mem_write(a.lin(a.ds, 0xC8FC), bytes([first, k]))
    a.mu.mem_write(a.lin(a.ds, 0x8FA6), bytes([ems]))
    a.w16(0xEB89, buf)
    a.mu.mem_write(a.lin(a.ds, 0x44F0), slots(fill))
    a.mu.mem_write(a.lin(a.ds, 0x45F0), kept[:8])
    a.mu.mem_write(a.lin(a.ds, 0x1044), kept[8:])
    for i, w in enumerate(table):
        a.w16(0x8FAA + 2 * i, w)
    a.mu.mem_write(a.lin(a.ds, LIST), b''.join(x + b'\0' for x in names) + b'\0')
    before = a.seg_bytes(a.ds)
    try:
        a.call_far(0x23BA, 0x0217, si=LIST)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    tab = ''.join(f'{w:04x}' for w in table)
    nm = ','.join(x.decode() for x in names) or '-'
    out.write(f'load | {on:x} {first:02x}{k:02x} {ems:x} {buf:x} {kept.hex()} {fill:x} {tab} {nm} | {res}\n')

for n in range(LOADS, LOADS + INFS):
    mif = rng.choice(INFO_MIFS)
    first = rng.choice([rng.randint(0, 0x1F), 0x0D, rng.randint(0x10, 0x16)])
    k = rng.choice([1, 2, 3, rng.randint(1, 8)])
    if not mine(n, total):
        continue
    a.restore(BASE)
    a.mu.mem_write(a.lin(a.ds, NAME), mif.lower() + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xBB0B), b'\xff')
    a.w16(0xC39A, 0x400)
    a.w16(0xCB84, 0)
    a.w16(0xCB88, 0x8000)
    a.mu.mem_write(a.lin(a.ds, 0xC8EF), b'\x80')
    try:
        a._setup([0, STUB_SEG], {'si': NAME})
        a._run(0, 0xE87C, STUB_SEG, 0)
    except (Fault, AssertionError):
        continue
    if a.r16(0xCB84) != INFO:
        continue
    a.w16(0x8F64, 1)
    a.mu.mem_write(a.lin(a.ds, 0xC8FC), bytes([first, k]))
    a.w16(0xCD4A, 0x0217)
    a.w16(0xCD4C, LOAD_SEG + 0x23BA)
    before = a.seg_bytes(a.ds)
    try:
        a.call_near(0, 0xEC03, si=INFO)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'inf | {mif.lower().decode()} {first:02x}{k:02x} | {res}\n')
for n in range(LOADS + INFS, total):
    ax = rng.choice([rng.randint(0, 0x19)] * 8 + [rng.randint(0x1A, 0x60), 0xFF])
    ems = rng.choice([1] * 10 + [0])
    if not mine(n, total):
        continue
    a.restore(BASE)
    a.mu.mem_write(a.lin(a.ds, 0x8FA6), bytes([ems]))
    before = a.seg_bytes(a.ds)
    try:
        a.call_far(0x23BA, 0x01C7, ax=ax)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'fight | {ax:x} {ems:x} | {res}\n')
out.close()
