"""Emit test vectors for the rented room's file (src/tavern.rs
`room_file_name` 1194:BA44, `room_read` C04C, `room_write` BA91) from the
original code.

Usage: gen_roomfile.py ACD_unpacked.EXE GAME_DIR OUT   (SHARD=k/K: par.py)

A case fills from a seed (`setup`, mirrored in tests/roomfile.rs) what
the name is made from: 5C8 (bit 4000h: the wild square's seed instead),
FD2, the town's taverns (the far pointer at 1AB5 to CSEG, their count
1387: a key that is there or isn't), the wild's 608, 60A, 610, 612; the
room's hours (1019), end (12CA) and the clock (5CC); a stale name at
733A; the path buffers' turn (1AEA). Then a script of the routines:

  0 write, read        1 read        2 write, hours and end anew, write,
  read                 3 write, hours 0, write (the delete), read

The files live in memory (gen_store.py's `StoreDos`). Each read's carry
is logged.

  roomfile | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (the open file's name's segment A90A put back
image-relative), of the carries, and of the files at the end (each name,
its length and bytes, in name order; and the names deleted and not made
again, in name order).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from unicorn.x86_const import UC_X86_REG_EFLAGS
from emu import Fault, DGROUP, changes, fnv, mine
from gen_attack import stream
from gen_store import StoreDos

CSEG = 0x6C00


def setup(seed):
    """(writes, cseg bytes, ops, the new hours and end of op 2). Mirrored
    in tests/roomfile.rs, draw for draw."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pd = lambda v: struct.pack('<I', v & 0xFFFFFFFF)
    w = []
    w.append((0x05C8, pk(0x4000 if byte() < 0x40 else 0)))
    fd2 = word()
    w.append((0x0FD2, pk(fd2)))
    n = byte() % 7
    w.append((0x1387, pk(n)))
    w.append((0x1AB5, pk(0)))
    w.append((0x1AB7, pk(CSEG)))
    key = (fd2 & 0xFF00) | ((fd2 & 0xFF) >> 1)
    there = byte() < 0xC0
    cseg = b''
    at = byte() % max(n, 1)
    for i in range(n):
        k = key if there and i == at else word()
        cseg += pk(k) + pk(word())
    for off in (0x0608, 0x060A, 0x0610, 0x0612):
        w.append((off, pk(word() if byte() < 0x80 else word() % 0x4000)))
    w.append((0x1019, pk(0 if byte() < 0x30 else word() % 0x200)))
    clock = dword()
    w.append((0x05CC, pd(clock)))
    # A quarter of the time at the read's 3Ch minutes' edge.
    edge = byte() < 0x40
    w.append((0x12CA, pd(clock + (0x3B + byte() % 3 if edge else (word() % 0x100) - 0x40))))
    stale = b'in' + bytes(0x30 + byte() % 10 for _ in range(4)) + b'.64\0'
    w.append((0x733A, stale))
    w.append((0x1AEA, bytes([byte() & 1 and 0xFF])))
    op = byte() % 4
    again = (word() % 0x200, dword())
    return w, cseg, op, again


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    a = StoreDos(EXE, GAME)
    mu = a.mu
    snap = a.snapshot()
    out = open(OUT, 'w', buffering=1)
    rng = random.Random(0x1994_BA91)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(snap)
        a.mh = {}
        a.deleted = []
        a.files = {}
        a.handles[5] = [b'', 0]
        writes, cseg, op, (hours, end) = setup(seed)
        for off, data in writes:
            mu.mem_write(a.lin(a.ds, off), data)
        mu.mem_write(a.lin(CSEG, 0), cseg + bytes(4))
        before = a.seg_bytes(a.ds)
        carries = []

        def write():
            a.call_near(0x1194, 0xBA91, es=a.ds)

        def read():
            a.call_near(0x1194, 0xC04C, es=a.ds)
            carries.append(mu.reg_read(UC_X86_REG_EFLAGS) & 1)
        try:
            if op == 0:
                write()
                read()
            elif op == 1:
                read()
            elif op == 2:
                write()
                a.w16(0x1019, hours)
                mu.mem_write(a.lin(a.ds, 0x12CA), struct.pack('<I', end))
                write()
                read()
            else:
                write()
                a.w16(0x1019, 0)
                write()
                read()
            if a.fault is not None:
                raise Fault(a.fault)
            ds = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', ds, 0xA90A)[0] == a.ds:
                ds[0xA90A:0xA90C] = struct.pack('<H', DGROUP)
            fl = b''.join(k.encode() + b'\0' + struct.pack('<I', len(v)) + bytes(v) for k, v in sorted(a.files.items()))
            fl += b''.join(k.encode() + b'\0' for k in sorted(set(a.deleted) - set(a.files)))
            if os.environ.get('DUMP') == f'{seed:x}':
                c = changes(before, bytes(ds))
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
                print('files', {k: bytes(v).hex() for k, v in a.files.items()}, a.deleted, carries, file=sys.stderr)
            res = ' '.join(f'{d:x}' for d in (fnv(changes(before, bytes(ds))), fnv(bytes(carries)), fnv(fl)))
        except (Fault, AssertionError, UcError):
            res = 'stuck'
        out.write(f'roomfile | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
