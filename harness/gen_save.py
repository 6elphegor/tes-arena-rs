"""Emit test vectors for src/save.rs from the original code.

Usage: gen_save.py ACD_unpacked.EXE GAME_DIR SAVE_DIR SLOT OUT

SAVE_DIR holds a saved game (SAVEENGN.nn for slot SLOT); it is searched
after GAME_DIR, as the port's Files::add_dir does. Vectors hold digests
only, never save data.

Lines:
  decrypt | n seed | digest      25FD:0832 on n LCG bytes at DGROUP A000
  path    | toggle name | digest 25FD:0844
  engine  | slot | digests       25FD:028D..029F, 035A..03F8: changed
                                 DGROUP bytes, creature block, buffer
                                 block, extra block
  load    | name off | ax digests  0000:7428 into LOAD_SEG:off: AX, changed
                                 DGROUP bytes, 64 KiB at LOAD_SEG
  spells  | slot | digests       29CF:0691: changed DGROUP bytes, segment
                                 3276:0000..3520
  level   | slot noscreen c398 planes | digests   25FD:0844 on SAVEGAME's
                                 name, then 0000:A9A6..AA4A (0000:946A and
                                 9B56, which write to the VGA, stubbed):
                                 changed DGROUP bytes, the three planes,
                                 the screen buffer
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG

EXE, GAME, SAVE, SLOT, OUT = sys.argv[1:6]
rng = random.Random(0x1994_25FD)
out = open(OUT, 'w', buffering=1)
CREATURES, BUFFER, EXTRA = 0x7000, 0x7400, (0x8800, 0x0010)


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


def lcg_bytes(n, seed):
    """n bytes from a 32-bit LCG (mirrored in tests/save.rs)."""
    x, b = seed, bytearray()
    for _ in range(n):
        x = (x * 1103515245 + 12345) & 0xffffffff
        b.append(x >> 16 & 0xff)
    return bytes(b)


a = DosArena(EXE, GAME, SAVE)
for off in (0x946A, 0x9B56):  # screen copy and palette: VGA only
    a.patch(0, off, b'\xc3')
a.init_heap()
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
BASE = a.snapshot()

for _ in range(200):
    a.restore(BASE)
    n = rng.choice([1, 2, 15, 16, 17, 0xE4F, rng.randint(1, 0x2000)])
    seed = rng.getrandbits(32)
    a.mu.mem_write(a.lin(a.ds, 0xA000), lcg_bytes(n, seed))
    a.call_near(0x25FD, 0x0832, esi=0xA000, ecx=n)
    out.write(f'decrypt | {n:x} {seed:x} | {fnv(a.mu.mem_read(a.lin(a.ds, 0xA000), n)):x}\n')

for name in (0x5E92, 0x5E9E, 0x5EC6, 0x7D06):
    for toggle in (0, 0xFF, 1):
        a.restore(BASE)
        a.mu.mem_write(a.lin(a.ds, 0x1AEA), bytes([toggle]))
        before = a.seg_bytes(a.ds)
        a.call_far(0x25FD, 0x0844, esi=name)
        after = a.seg_bytes(a.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        out.write(f'path | {toggle:x} {name:x} | {fnv(changed):x} {a.mu.reg_read(__import__("unicorn").x86_const.UC_X86_REG_SI):x}\n')

for slot in (int(SLOT), int(SLOT) + 1):
    a.restore(BASE)
    a.w16(0x1042, CREATURES)
    a.w16(0x9030, BUFFER)
    a.w16(0x1020, EXTRA[1])
    a.w16(0x1022, EXTRA[0])
    before = a.seg_bytes(a.ds)
    try:
        a.run_range(0x25FD, 0x028D, 0x02A0, eax=slot)
        a.run_range(0x25FD, 0x035A, 0x03F9)
        if a.r16(0xA90A) == a.ds:
            a.w16(0xA90A, a.ds - LOAD_SEG)
        after = a.seg_bytes(a.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        blocks = [a.mu.mem_read(a.lin(CREATURES, 0), 0x20F0), a.mu.mem_read(a.lin(BUFFER, 0xA000), 0x15E0),
                  a.mu.mem_read(a.lin(EXTRA[0], EXTRA[1]), 0x120)]
        res = ' '.join(f'{fnv(b):x}' for b in [changed] + blocks)
    except Fault:
        res = 'fault'
    out.write(f'engine | {slot:x} | {res}\n')

LOAD = 0x7800
names = ['CLASSES.DAT', 'NAMECHNK.DAT', 'CITYDATA.65', 'SPELLSG.65', 'BS5.MIF', 'nosuch.dat',
         'GLOBAL.BSA', 'TVBD1A.MIF', 'ARENA.BAT', 'START.MIF']
for name in names:
    for off in (0, 0x100):
        a.restore(BASE)
        a.mu.mem_write(a.lin(a.ds, 0xF000), name.encode() + b'\0')
        before = a.seg_bytes(a.ds)
        try:
            r = a.call_far(0, 0x7428, esi=0xF000, es=LOAD, edi=off)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            from unicorn.x86_const import UC_X86_REG_EFLAGS
            cf = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
            res = 'fault' if cf else f'{r:x} {fnv(changed):x} {fnv(a.seg_bytes(LOAD)):x}'
        except (Fault, AssertionError):
            res = 'fault'
        out.write(f'load | {name} {off:x} | {res}\n')

for slot in (int(SLOT), int(SLOT) + 1):
    a.restore(BASE)
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), bytes([slot]))
    before = a.seg_bytes(a.ds)
    try:
        a.call_far(0x29CF, 0x0691)
        if a.r16(0xA90A) == a.ds:
            a.w16(0xA90A, a.ds - LOAD_SEG)
        after = a.seg_bytes(a.ds)
        changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
        res = f'{fnv(changed):x} {fnv(a.mu.mem_read(a.lin(LOAD_SEG + 0x3276, 0), 0x3520)):x}'
    except (Fault, AssertionError):
        res = 'fault'
    out.write(f'spells | {slot:x} | {res}\n')

SCREEN = 0x6000
for slot in (int(SLOT), int(SLOT) + 1):
    for noscreen in (0, 1):
        for planes in (0, 1):
            a.restore(BASE)
            c398 = rng.getrandbits(16)
            a.run_range(0x25FD, 0x028D, 0x02A0, eax=slot)
            a.w16(0x9030, BUFFER)
            a.w16(0xA8FE, 0)
            a.w16(0xA900, SCREEN)
            a.mu.mem_write(a.lin(a.ds, 0xC995), bytes([noscreen]))
            a.w16(0xC398, c398)
            if planes:
                for var in (0xD806, 0xCCD0, 0xD748):
                    r = a.call_near(0, 0x51EC, eax=0x8000)
                    a.w16(var, r['dx'])
            before = a.seg_bytes(a.ds)
            try:
                a.call_far(0x25FD, 0x0844, esi=0x5E92)
                from unicorn.x86_const import UC_X86_REG_SI
                si = a.mu.reg_read(UC_X86_REG_SI)
                a.run_range(0, 0xA9A6, 0xAA4B, esi=si)
                if a.r16(0xA90A) == a.ds:
                    a.w16(0xA90A, a.ds - LOAD_SEG)
                after = a.seg_bytes(a.ds)
                changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
                parts = [fnv(changed)]
                for var in (0xD806, 0xCCD0, 0xD748):
                    sg = a.r16(var)
                    parts.append(fnv(a.mu.mem_read(a.lin(sg, 0), 0x8000)) if sg else 0)
                parts.append(fnv(a.mu.mem_read(a.lin(SCREEN, 0), 0xFA00)))
                res = ' '.join(f'{p:x}' for p in parts)
            except (Fault, AssertionError):
                res = 'fault'
            out.write(f'level | {slot:x} {noscreen:x} {c398:x} {planes:x} | {res}\n')
out.close()
