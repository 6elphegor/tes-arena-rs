"""Emit test vectors for item names (src/inventory.rs item_name,
1194:7390 and 27CE:000C) from the original code.

Usage: gen_items.py ACD_unpacked.EXE OUT

Each case writes a 19-byte item slot at SEG:OFF (DGROUP or another
segment) and runs 1194:7390 with ES:SI on it. Recorded: the returned SI,
and FNV-1a digests of the 64 bytes at DGROUP:SI (the name as the caller
reads it, up to its NUL), of the 96 bytes at SEG:1395 (where formatted names are
built) and of the changed DGROUP bytes.

Lines: item | seg off slot_hex | si digests, or "fault"/"hang"
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes
from unicorn import UcError

EXE, OUT = sys.argv[1:3]
rng = random.Random(0x1994_7390)
out = open(OUT, 'w', buffering=1)
OTHER = 0x7000


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


a = Arena(EXE)
# DGROUP's relocated words hold segment values that differ from the
# port's (image at 0). The name code reads DGROUP only as strings and a
# table of near offsets, never through a far pointer, so DGROUP is put
# back to image-relative values: the walks over it (a type of 0x80 or
# more skips nearly 65536 strings) then see the same bytes as the port.
_b = open(EXE, 'rb').read()
_h = struct.unpack('<14H', _b[:28])
for o, sg in struct.iter_unpack('<2H', _b[_h[12]:_h[12] + 4 * _h[3]]):
    lin = sg * 16 + o - 0x3BA0 * 16
    if 0 <= lin < 0xFFFF:
        v = a.r16(lin)
        a.w16(lin, v - LOAD_SEG)
BASE = bytes(a.mu.mem_read(0, 0x110000))

for i in range(1500):
    a.mu.mem_write(0, BASE)
    in_dg = rng.random() < 0.7
    seg_img = 0x3BA0 if in_dg else OTHER       # image-relative, as in the port
    seg = a.ds if in_dg else OTHER             # what the emulator uses
    off = rng.choice([0xF000, 0x0212 + 0x13 * rng.randrange(40)]) if in_dg else rng.randrange(0, 0xF000)
    ty = rng.choice([rng.randint(0, 20), rng.randint(0, 20), rng.getrandbits(8)])
    flags = rng.getrandbits(8)
    if rng.random() < 0.6:
        flags &= 0xFD  # identified
    slot = bytearray(rng.getrandbits(8) for _ in range(19))
    slot[0] = ty
    slot[0xE] = flags
    slot[0xF] = rng.choice([0xFF, rng.randint(0, 8), rng.getrandbits(8)])
    slot[0x10] = rng.choice([0xFF, rng.randint(0, 7), rng.randint(0, 3), rng.getrandbits(8)])
    a.mu.mem_write(a.lin(seg, off), bytes(slot))
    before = a.seg_bytes(a.ds)
    try:
        r = a.call_near(0x1194, 0x7390, es=seg, esi=off)
        from unicorn.x86_const import UC_X86_REG_SI
        si = a.mu.reg_read(UC_X86_REG_SI)
        after = a.seg_bytes(a.ds)
        changed = changes(before, after)
        # The name up to its NUL (at most 64 bytes): past it lie
        # relocated segment words that differ from the port's.
        name = (after + after)[si:si + 64].split(b'\0')[0]
        buf = bytes(a.mu.mem_read(a.lin(seg, 0x1395), 96))
        res = f'{si:x} {fnv(name):x} {fnv(buf):x} {fnv(changed):x}'
    except (Fault, UcError):
        # UcError: a jump through a table entry that isn't code.
        res = 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'item | {seg_img:x} {off:x} {slot.hex()} | {res}\n')
out.close()
