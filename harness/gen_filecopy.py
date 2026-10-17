"""Emit the vectors for a file's copy (25FD:056F, `Machine::file_copy`),
with its source there and missing.

Usage: gen_filecopy.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The game started and set up as gen_programstart.py has it, then for each
case from that state: the names written at DGROUP 1044 (from) and 1051
(to) and 056F far-called with SI 1044, DI 1051. One line a case:

  copy | NAME | FROM | TO | <runs> | <digest> | <written> | <ems>

what it changed in DGROUP (the open file's name's segment, A90A, taken
1000h lower as the port's), the destination as written (its bytes'
length and digest, or `-` not written) and the 32 EMS banks' digest.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena
from gen_msgbox import runs

CASES = [
    ('there', b'LOG.01', b'LOG.99'),
    ('missing', b'NOSUCH.01', b'NOSUCH.99'),
    ('missing-over', b'NOSUCH.02', b'STATES.01'),
]


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    a.w16(0xCB8A, a.r16(0x9030))
    saved = a.stub()
    a.mu.mem_write(a.lin(a.ds, 0x1A80), b'\x40')
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), b'\x40')
    assert a.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
    a.unstub(saved)
    snap = a.snapshot()
    out = open(OUT, 'w')
    for name, src, dst in CASES:
        a.restore(snap)
        a.mu.mem_write(a.lin(a.ds, 0x1044), src + b'\0')
        a.mu.mem_write(a.lin(a.ds, 0x1051), dst + b'\0')
        d0 = bytearray(a.seg_bytes(a.ds))
        assert a.run_screen(0x25FD, 0x056F, dict(si=0x1044, di=0x1051)) is None, f'{name}: the copy waited'
        d1 = bytearray(a.seg_bytes(a.ds))
        for d in (d0, d1):
            if struct.unpack_from('<H', d, 0xA90A)[0] == a.ds:
                struct.pack_into('<H', d, 0xA90A, a.ds - 0x1000)
        offs, digest = runs(d0, d1)
        w = a.written.get(dst.decode('latin1'))
        written = '-' if w is None else f'{len(w):x} {fnv(bytes(w)):x}'
        ems = fnv(b''.join(a.ems_bank(b) for b in range(0x20)))
        out.write(f'copy | {name} | {src.decode()} | {dst.decode()} | {offs} | {digest} | {written} | {ems:x}\n')
        out.flush()
    out.close()


if __name__ == '__main__':
    main()
