"""Emit test vectors for the sky's images loaded on entering an area
outdoors (src/skyobj.rs `sky_images`, 0000:E5C9, with the star field
0000:E77A) from the original code.

Usage: gen_skyimg.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer (harness/dos.py) with EMS on: 32 banks of
64 KB swapped through the frame at 8800h. A case sets (from a seed,
`setup`, mirrored in tests/skyimg.rs) the map flags (C398: outdoors,
bit 4, mostly), the sky's bank (C8F3: mostly the image's 0Eh, else
another or a negative byte), the area's seed (CB6E), the game's seed
(91F6) and the bright stars' marks (D89F..D8A1), then far-calls 0000:E5C9.
The star field's INT 3 (0000:E83B, where its clusters overrun their
table) is passed over, as DOS's handler does.

  skyimg | seed | digests

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
    """(offset, bytes) writes into DGROUP. Mirrored in tests/skyimg.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    w = []
    flags = word() & ~4 | (4 if byte() < 0xE0 else 0)
    w.append((0xC398, struct.pack('<H', flags)))
    k = byte()
    bank = 0x0E if k < 0xC0 else (byte() % 0x20 if k < 0xF0 else 0x80 | byte())
    w.append((0xC8F3, bytes([bank])))
    w.append((0xCB6E, struct.pack('<I', dword())))
    w.append((0x91F6, struct.pack('<I', dword())))
    w.append((0xD89F, bytes(byte() & 1 for _ in range(3))))
    return w


def unrelocate(a, b):
    """DGROUP bytes with the open name's segment (A90A) image-relative."""
    b = bytearray(b)
    if struct.unpack_from('<H', b, 0xA90A)[0] == a.ds:
        struct.pack_into('<H', b, 0xA90A, a.ds - LOAD_SEG)
    return b


def main():
    a = DosArena(EXE, GAME)
    a.GAME_INT3 = set(a.GAME_INT3) | {(0x0000, 0xE83C)}
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    base = a.snapshot()
    rng = random.Random(0x1994_E5C9)
    out = open(OUT, 'w', buffering=1)
    total = int(os.environ.get('N', 1000))
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
            a._run(0, 0xE5C9, STUB_SEG, 0)
            after = unrelocate(a, a.seg_bytes(a.ds))
            banks = b''.join(a.ems_bank(b) for b in range(0x20))
            res = f'{fnv(changes(before, after)):x} {fnv(banks):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'skyimg | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
