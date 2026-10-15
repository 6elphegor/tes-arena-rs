"""Emit test vectors for the store's list lines (src/store.rs
`store_lines`, 1194:F142 weapons and F204 armour) from the original code.

Usage: gen_storelines.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/storelines.rs) the
records in the EMS frame (weapons from 0, armour from 7D0, 13 bytes each:
the magic byte +1, the kind +2, the armour kind +3, the material +4, the
type +5 (now and then past the tables), the suffix +6, the price +7),
their counts (12C6, 12CF; sometimes none), the shop's holiday (13C9) and
the date (5CC) its price reads, and leftovers in the template (13F3), the
name (1395), the weight's text (10A8) and A874. The frame (9030) is a
scratch segment. Everything runs: the naming (27CE:000C), the sprintf
(0000:8D14), the holiday's price (1194:4A6B).

  storelines | seed | op | result

Op: 0 weapons (F142), 1 armour (F204). Results: "stuck" (a fault or a
hang: a weight F2FC never finds, no records), or the FNV-1a digests of
the changed DGROUP bytes and of the frame.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream

FSEG = 0x7000


def setup(seed):
    """(op, writes, frame bytes). Mirrored in tests/storelines.rs, draw
    for draw (no tuple draws two)."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() & 1
    w = []
    frame = bytearray(0x1000)
    count = 0 if byte() < 0x08 else 1 + byte() % 10
    for i in range(count):
        r = bytearray(13)
        r[0] = byte()
        r[1] = (0, 0, 1, 1, 2)[byte() % 5] if op == 0 else (0, 0, 1, 3)[byte() % 4]
        r[2] = (0, 1, 1, 2)[byte() % 4]
        r[3] = byte() % 4
        # An accessory's (kind 2) material picks its namer from three
        # (27CE:004F); past them the original calls what follows the table.
        if r[2] == 2:
            r[4] = byte() % 3
        else:
            r[4] = 0xFF if byte() < 0x40 else byte() % 8
        r[5] = byte() % (0x14 if op == 0 else 0x0C) if byte() < 0xF0 else byte()
        r[6] = 0xFF if byte() < 0x40 else byte() % 0x20
        r[7:11] = struct.pack('<I', word() if byte() < 0xE0 else word() | byte() << 16)
        r[11] = byte()
        r[12] = byte()
        frame[13 * i:13 * i + 13] = r
    w.append((0x12C6 if op == 0 else 0x12CF, bytes([count])))
    w.append((0x13C9, bytes([byte() & 1])))
    w.append((0x05CC, bytes(byte() for _ in range(4))))
    w.append((0x13F3, bytes(byte() for _ in range(13))))
    w.append((0x1395, bytes(byte() for _ in range(16))))
    w.append((0x10A8, bytes(byte() for _ in range(8))))
    w.append((0xA874, pk(word())))
    return op, w, bytes(frame)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    # DGROUP's relocated words put back image-relative, as the port holds
    # them (a name read past its list meets them).
    b = open(EXE, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    for o, sg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
        lin = sg * 16 + o - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            a.w16(lin, a.r16(lin) - LOAD_SEG)
    # All of memory put back for each case: a garbage path can write
    # anywhere (code included), which later cases would run.
    whole = bytes(a.mu.mem_read(0, 0x110000))
    rng = random.Random(0x1994_F142)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(0, whole)
        a.w16(0x9030, FSEG)
        op, writes, frame = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        a.mu.mem_write(a.lin(FSEG, 0 if op == 0 else 0x7D0), frame)
        before = (a.seg_bytes(a.ds), a.seg_bytes(FSEG))
        try:
            # The near routine from the same registers each time (_setup),
            # and the flags: a garbage path can leave TF set, which traps
            # every later case at its first instruction.
            from unicorn.x86_const import UC_X86_REG_EFLAGS
            a.mu.reg_write(UC_X86_REG_EFLAGS, 0x0002)
            a.call_near(0x1194, (0xF142, 0xF204)[op])
            res = f'{fnv(changes(before[0], a.seg_bytes(a.ds))):x} {fnv(changes(before[1], a.seg_bytes(FSEG))):x}'
            if os.environ.get('DUMP') == f'{seed:x}':
                for name, b0, b1 in (('ds', before[0], a.seg_bytes(a.ds)), ('fr', before[1], a.seg_bytes(FSEG))):
                    c = changes(b0, b1)
                    for i in range(0, len(c), 3):
                        o, v = struct.unpack_from('<HB', c, i)
                        print(f'{name} {o:04x} {b0[o]:02x} {v:02x}', file=sys.stderr)
        except (Fault, AssertionError, UcError) as e:
            res = 'stuck'
            if os.environ.get('DEBUG'):
                from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP
                print(f'{seed:x}: {e!r} at {a.mu.reg_read(UC_X86_REG_CS) - LOAD_SEG:04x}:{a.mu.reg_read(UC_X86_REG_IP):04x}', file=sys.stderr)
        out.write(f'storelines | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
