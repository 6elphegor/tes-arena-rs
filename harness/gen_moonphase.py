"""Emit test vectors for a moon's phase made (src/celestial.rs
`moon_phase_load`, 0000:E6CB).

Usage: gen_moonphase.py ACD_unpacked.EXE GAME_DIR OUT [N]

Runs under the DOS file layer (harness/dos.py) with the user's files,
GLOBAL.BSA open, and EMS on with the frame at FRAME. A case (seed = its
number, `setup`, mirrored in tests/moonphase.rs) picks the moon (SI D871h
"MOON01.DFA" or D87Ch "MOON02.DFA"), the frame CX (mostly a day 0..1Fh,
at times past the file's frames), DI (mostly in the frame's first 60 KB,
at times anywhere, as the second moon's CCAEh is) and the load buffer
[BA54] (BUF:0); the frame segment is filled with a byte of the case first.

  moonphase | seed | result

result: "fault", or the FNV-1a digests of the frame segment after and of
the changed DGROUP bytes.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from dos import DosArena
from emu import Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

FRAME = 0x8800
BUF = 0x7000


def setup(seed):
    """SI, CX, DI and the frame's fill. Mirrored in tests/moonphase.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    si = 0xD871 if byte() & 1 == 0 else 0xD87C
    k = byte()
    cx = byte() % 0x20 if k < 0xE0 else byte()
    k = byte()
    di = word() % 0xF000 if k < 0xE0 else word()
    fill = byte()
    return si, cx, di, fill


def main():
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    total = int(sys.argv[4]) if len(sys.argv) > 4 else 500
    a = DosArena(exe, game)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    a.w16(0xBA54, 0)
    a.w16(0xBA56, BUF)
    base = a.snapshot()
    with open(out, 'w') as f:
        for n in range(total):
            si, cx, di, fill = setup(n)
            if not mine(n, total):
                continue
            a.restore(base)
            a.mu.mem_write(a.lin(FRAME, 0), bytes([fill]) * 0x10000)
            ds0 = a.seg_bytes(a.ds)
            try:
                a.call_near(0, 0xE6CB, esi=si, ecx=cx, edi=di)
                # The last load's segment, kept at A90A: DGROUP's, the
                # image's as the port's (gen_text.py).
                if a.r16(0xA90A) == a.ds:
                    a.w16(0xA90A, a.ds - LOAD_SEG)
                ds1 = a.seg_bytes(a.ds)
                r = '%x %x' % (fnv(a.seg_bytes(FRAME)), fnv(changes(ds0, ds1)))
            except (Fault, UcError, AssertionError):
                r = 'fault'
            f.write(f'moonphase | {n:x} | {r}\n')


if __name__ == '__main__':
    main()
