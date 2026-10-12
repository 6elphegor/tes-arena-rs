"""Emit test vectors for src/level.rs (0000:E87C) and the whole city
(1194:0292) from the original code.

Usage: gen_level.py ACD_unpacked.EXE GAME_DIR OUT [SKIP]

SKIP resumes an interrupted run: the first SKIP cases draw their
settings but are not run, and the rest are appended to OUT. (Levels past
the last one run to the 50M-instruction budget, so a full run is slow.)

Runs under the DOS file layer (harness/dos.py) with the user's game
directory, as gen_mif.py does. The heap stub hands out blocks from
paragraph 0xA000 and nothing is freed during a case, so tests/level.rs
mirrors it with `mem::Bump(0xA000)`. Vectors hold only names, settings
and FNV-1a digests: of the changed DGROUP bytes (offset and value) and
of the floor, wall, ceiling and buffer segments the DGROUP pointers
name after the call.

Two harness choices keep the comparison to what the port models:
- The decompressor (3065:0D2D, compiled C) mallocs seven tables through
  0000:51D3 and frees them before returning; the port keeps them
  internally. Here 51D3 allocates from a scratch arena (paragraph
  0xE000) so the planes get the same segments as with `Bump`, and the
  table pointers it leaves in DGROUP (EEBA..EEE1) are left out.
- Planes are digested over their 32 KB blocks; the buffer segment over
  64 KB.
The far pointer to the file name (A908) holds DGROUP's relocated segment
and is converted back to image-relative.

Lines (fields separated by " | "):
  level | name override flags info map2 text map_flags | digests
  city  | template x y dim prov start seed plan | digests
The result is "fault" for a CPU fault and "hang" when the instruction
budget runs out (levels past the last one; see CHUNK_LIMIT in
src/level.rs), which the test only requires the port to survive.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG
from unicorn.x86_const import UC_X86_REG_SP, UC_X86_REG_SS

EXE, GAME, OUT = sys.argv[1:4]
SKIP = int(sys.argv[4]) if len(sys.argv) > 4 else 0
rng = random.Random(0x1994_E87C)
out = open(OUT, 'a' if SKIP else 'w', buffering=1)  # line-buffered: progress is visible
done = 0


def skip():
    """True for cases an interrupted run already wrote."""
    global done
    done += 1
    return done <= SKIP

BUFFER_SEG, MAP2_PRESET = 0x8800, 0x8000
PLANE_VARS = (0xCCD0, 0xD806, 0xD748, 0x9030)
NAME = 0x0A36


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


SCRATCH = 0xE000
LZ_PTRS = range(0xEEBA, 0xEEE2)


class LevelArena(DosArena):
    """51D3 (C malloc, size on the stack) serves from a scratch arena,
    which starts over once everything in it is freed (6C26); the
    decompressor frees all its tables before returning."""

    def _intr(self, mu, intno, x):
        if intno == 0xF4:
            sp = self._reg('sp')
            size = struct.unpack('<H', mu.mem_read(self.lin(self._reg('ss'), sp + 4), 2))[0]
            self._wreg('dx', self.scratch)
            self._wreg('ax', 0)
            self.live.add(self.scratch)
            self.scratch += (size + 15) >> 4
            assert self.scratch < 0x10F00, 'scratch arena exhausted'
            return
        if intno == 0xF2:
            sp = self._reg('sp')
            seg = struct.unpack('<H', mu.mem_read(self.lin(self._reg('ss'), sp + 6), 2))[0]
            if seg in self.live:
                self.live.remove(seg)
                if not self.live:
                    self.scratch = SCRATCH
                return
        super()._intr(mu, intno, x)


a = LevelArena(EXE, GAME)
a.patch(0, 0x51D3, b'\xcd\xf4\xcb')
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
a.w16(0x9030, BUFFER_SEG)
BASE = a.snapshot()

bsa = open(os.path.join(GAME, a.names['GLOBAL.BSA']), 'rb').read()
count = struct.unpack_from('<H', bsa, 0)[0]
d = len(bsa) - 18 * count
NAMES = [bsa[d + 18 * i: d + 18 * i + 14].split(b'\0')[0] for i in range(count)]
MIFS = sorted({n.upper() for n in NAMES if n.upper().endswith(b'.MIF')}
              | {n.upper().encode() for n in os.listdir(GAME) if n.upper().endswith('.MIF')})


def fresh():
    a.restore(BASE)
    a.reset_heap()
    a.scratch = SCRATCH
    a.live = set()


def run(seg, off, si):
    a._setup([0, STUB_SEG], {'si': si})
    a._run(seg, off, STUB_SEG, 0)  # 50M-instruction budget


def digests(before):
    if a.r16(0xA90A) == a.ds:
        a.w16(0xA90A, a.ds - LOAD_SEG)
    after = a.seg_bytes(a.ds)
    changed = bytearray()
    for i in range(0x10000):
        if after[i] != before[i] and i not in LZ_PTRS:
            changed += struct.pack('<HB', i, after[i])
    segs = [a.r16(v) for v in PLANE_VARS]
    sizes = (0x8000, 0x8000, 0x8000, 0x10000)
    return f'{fnv(changed):x} ' + ' '.join(
        f'{fnv(a.seg_bytes(s)[:n]) if s else 0:x}' for s, n in zip(segs, sizes))


# --- single level loads ------------------------------------------------------

cases = [(n, 0xFF) for n in MIFS]
cases += [(rng.choice(MIFS), rng.choice([0, 1, 2, 3, 5, 0x40])) for _ in range(200)]
cases += [(b'NOSUCH.MIF', 0xFF), (b'', 0xFF)]
for name, override in cases:
    fresh()
    flags = rng.choice([0, 0x200, rng.getrandbits(16)])
    info = rng.choice([0, 0, 0x4718])
    map2 = rng.choice([0, MAP2_PRESET])
    text = rng.choice([0x8000, rng.getrandbits(16)])
    map_flags = rng.getrandbits(16)
    nm = bytes(c | 0x20 if rng.random() < 0.5 and 0x41 <= c <= 0x5a else c for c in name)
    if skip():
        continue
    a.mu.mem_write(a.lin(a.ds, NAME), nm + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0xBB0B), bytes([override]))
    a.w16(0xC39A, flags)
    a.w16(0xCB84, info)
    a.w16(0xD748, map2)
    a.w16(0xCB88, text)
    a.w16(0xC398, map_flags)
    before = a.seg_bytes(a.ds)
    try:
        run(0, 0xE87C, NAME)
        res = digests(before)
    except Fault:
        res = 'fault'
    except AssertionError:
        from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP
        print('hang', nm, override, hex(a.mu.reg_read(UC_X86_REG_CS)), hex(a.mu.reg_read(UC_X86_REG_IP)),
              hex(a.r16(0xC357)), hex(a.r16(0xC359)), a.mu.mem_read(a.lin(a.ds, 0xCCFA), 4), flush=True)
        res = 'hang'
    out.write(f'level | {nm.decode()} {override:x} {flags:x} {info:x} {map2:x} {text:x} {map_flags:x} | {res}\n')


# --- whole cities ---------------------------------------------------------------

TEMPLATES = {6: [b'CITY%d.MIF' % i for i in range(1, 6)] + [b'CITYW%d.MIF' % i for i in range(1, 4)],
             5: [b'TOWN%d.MIF' % i for i in range(1, 6)],
             4: [n for n in MIFS if n.startswith(b'VI')]}
for _ in range(40):
    fresh()
    prov = rng.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 8])
    dim = rng.choice([4, 5, 6])
    template = b'IMPERIAL.MIF' if prov == 8 else rng.choice(TEMPLATES[dim])
    x, y = rng.getrandbits(16), rng.getrandbits(16)
    start = rng.choice([0, 0x28, 0x1428, 0x2850])
    plan = bytes(1 if rng.random() < 0.15 else 0 for _ in range(dim * dim))
    seed = rng.getrandbits(32)
    if skip():
        continue
    a.mu.mem_write(a.lin(a.ds, NAME), template.lower() + b'\0')
    a.mu.mem_write(a.lin(a.ds, 0x486B), plan)
    a.mu.mem_write(a.lin(a.ds, 0xA43), bytes([dim]))
    a.w16(0xA44, start)
    a.mu.mem_write(a.lin(a.ds, 0xA46), bytes([prov]))
    a.w16(0xA48, x)
    a.w16(0xA4A, y)
    a.w32(0x91F6, seed)
    a.w16(0x91F0, 0)
    before = a.seg_bytes(a.ds)
    try:
        run(0x1194, 0x0292, 0)
        res = digests(before)
    except Fault:
        res = 'fault'
    out.write(f'city | {template.lower().decode()} {x:x} {y:x} {dim:x} {prov:x} {start:x} {seed:x} {plan.hex()} | {res}\n')
out.close()
