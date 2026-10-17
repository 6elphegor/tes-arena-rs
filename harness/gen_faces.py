"""Emit test vectors for the LZSS unpacker (src/faces.rs `lzss_unpack`,
0000:7608) and the faces over the view (`faces_load`, 292B:048F, and
`faces_draw`, 292B:0530).

Usage: gen_faces.py ACD_unpacked.EXE GAME_DIR OUT [N]

Runs under the DOS file layer (harness/dos.py) with the user's files,
GLOBAL.BSA open, the heap set up (A000h..F000h) and EMS on with the
frame at FRAME. Each case (seed = its number, `setup`, mirrored in
tests/faces.rs) is one of:

- lzss: a stream at SRC:0 (a length word, then random bytes, some of
  them made references back into what was written), unpacked to
  OUT:DI; the ring either the far pointer at ADBC (RING:off, filled
  with a byte) or, with ADBE 0, a heap block.
- faces: FACES (0FD5, eight bytes) each FFh or a face (0..79, bit 80h
  at times), 5CA bit 2000h at times set; 048F far-called with the load
  buffer [BA54] at BUF:0, then 0530 with the screen [A900] at SCREEN
  (filled with a byte) and the row table at A6A0 rows of 140h.

  lzss | seed | result
  faces | seed | result

result: "fault", or FNV-1a digests: lzss of OUT's segment, the ring's
segment, the heap's (A000h..F000h) and the changed DGROUP bytes; faces
of the frame (bank 19h) and the changed DGROUP bytes after 048F, then
the screen and the changed DGROUP bytes after 0530.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG, changes, fnv, mine
from gen_attack import stream

FRAME = 0x8800
BUF = 0x7000
SRC = 0x8000
OUT = 0x7000
RING = 0x5C00
SCREEN = 0x5C00


def setup(seed):
    """The case. Mirrored in tests/faces.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    if seed % 3 != 2:
        n = word() % 0x400 if byte() < 0xF0 else word() % 0x2000
        data = bytearray(byte() for _ in range(n))
        ring = byte() & 1 == 0
        ring_off = word() if byte() < 0x20 else word() % 0x100
        ring_fill = byte()
        di = word()
        return ('lzss', n, bytes(data), ring, ring_off, ring_fill, di)
    faces = []
    for _ in range(8):
        b = byte()
        faces.append(0xFF if b < 0x80 else (byte() % 80) | (0x80 if b & 1 else 0))
    done = byte() < 0x18
    fill = byte()
    return ('faces', faces, done, fill)


def main():
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    total = int(sys.argv[4]) if len(sys.argv) > 4 else 1500
    a = DosArena(exe, game)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    a.w16(0xBA54, 0)
    a.w16(0xBA56, BUF)
    a.w16(0xA900, SCREEN)
    for y in range(200):
        a.w16(0xA6A0 + 2 * y, y * 0x140)
    base = a.snapshot()
    with open(out, 'w') as f:
        for n in range(total):
            case = setup(n)
            if not mine(n, total):
                continue
            a.restore(base)
            try:
                if case[0] == 'lzss':
                    _, ln, data, ring, ring_off, ring_fill, di = case
                    a.w16(0, ln, seg=SRC)
                    a.mu.mem_write(a.lin(SRC, 2), data)
                    a.mu.mem_write(a.lin(RING, 0), bytes([ring_fill]) * 0x10000)
                    a.w16(0xADBC, ring_off)
                    a.w16(0xADBE, RING if ring else 0)
                    ds0 = a.seg_bytes(a.ds)
                    a._setup([0, STUB_SEG, 0, SRC, di, OUT], {})
                    a._run(0, 0x7608, STUB_SEG, 0, count=10_000_000)
                    heap = bytes(a.mu.mem_read(a.lin(0xA000, 0), 0x50000))
                    r = '%x %x %x %x' % (fnv(a.seg_bytes(OUT)), fnv(a.seg_bytes(RING)), fnv(heap),
                                         fnv(changes(ds0, a.seg_bytes(a.ds))))
                else:
                    _, faces, done, fill = case
                    a.mu.mem_write(a.lin(a.ds, 0x0FD5), bytes(faces))
                    if done:
                        a.w16(0x05CA, a.r16(0x05CA) | 0x2000)
                    a.mu.mem_write(a.lin(SCREEN, 0), bytes([fill]) * 0x10000)
                    ds0 = a.seg_bytes(a.ds)
                    a.call_far(0x292B, 0x048F)
                    if a.r16(0xA90A) == a.ds:
                        a.w16(0xA90A, a.ds - LOAD_SEG)
                    ds1 = a.seg_bytes(a.ds)
                    r1 = '%x %x' % (fnv(a.ems_bank(0x19)), fnv(changes(ds0, ds1)))
                    a.call_far(0x292B, 0x0530)
                    r = '%s %x %x' % (r1, fnv(a.seg_bytes(SCREEN)), fnv(changes(ds1, a.seg_bytes(a.ds))))
            except (Fault, UcError, AssertionError):
                r = 'fault'
            f.write(f'{case[0]} | {n:x} | {r}\n')


if __name__ == '__main__':
    main()
