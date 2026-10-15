"""Emit test vectors for the effects' flats loaded at the program's start
(src/info.rs `effect_flats`, 21EE:0134) from the original code.

Usage: gen_fxflats.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer (harness/dos.py) with EMS on: 32 banks of
64 KB swapped through the frame at 8800h. A case sets (from a seed,
`setup`, mirrored in tests/fxflats.rs) the effects' banks (C8FA: the
first, mostly the image's 0Fh; C8FB: how many, mostly 2), the bank table
at 8FAA (each bank's free count: mostly all free, else full, half or a
random word), the flat count (D432), a stretch of the kinds table and
the kinds pointer (CCC0), then far-calls
21EE:0134: the bank table kept at 929E and the other banks marked full
(0000:FEF4), the flats reset (BFA5), the 1Ch files named at 3B08 loaded
(BED4) and the other banks' entries put back (FECE).

  fxflats | seed | digests

Digests: FNV-1a of the changed DGROUP bytes (offset, value; the open
name's segment at A90A made image-relative), and of the 32 banks as the
program sees them. "fault" / "hang".
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG, changes, fnv, mine
from gen_attack import stream

EXE, GAME, OUT = sys.argv[1:4]
FRAME = 0x8800


def setup(seed):
    """(offset, bytes) writes into DGROUP. Mirrored in tests/fxflats.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    first = 0x0F if byte() < 0xC0 else byte() % 0x1E
    n = 2 if byte() < 0xC0 else 1 + byte() % 3
    w.append((0xC8FA, bytes([first, n])))
    table = bytearray()
    for _ in range(0x20):
        k = byte() % 8
        v = (0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0, word(), 0x8000)[k]
        table += struct.pack('<H', v)
    w.append((0x8FAA, bytes(table)))
    w.append((0xD432, struct.pack('<H', word() % 0x40)))
    w.append((0xCA97, bytes(byte() for _ in range(0xA0))))
    w.append((0xCCC0, struct.pack('<H', 0xCA97 + word() % 0x60)))
    return w


def main():
    a = DosArena(EXE, GAME)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    base = a.snapshot()
    rng = random.Random(0x1994_0134)
    out = open(OUT, 'w', buffering=1)
    total = int(os.environ.get('N', 800))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = unrelocate(a, a.seg_bytes(a.ds))
        try:
            a._setup([0, STUB_SEG], {})
            a._run(0x21EE, 0x0134, STUB_SEG, 0)
            after = unrelocate(a, a.seg_bytes(a.ds))
            banks = b''.join(a.ems_bank(b) for b in range(0x20))
            res = f'{fnv(changes(before, after)):x} {fnv(banks):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'fxflats | {seed:x} | {res}\n')
    out.close()


def unrelocate(a, b):
    """DGROUP bytes with the open name's segment (A90A) image-relative."""
    b = bytearray(b)
    if struct.unpack_from('<H', b, 0xA90A)[0] == a.ds:
        struct.pack_into('<H', b, 0xA90A, a.ds - LOAD_SEG)
    return b


if __name__ == '__main__':
    main()
