"""Emit the vectors for a whole file loaded into EMS (0000:7428 with DI
FFFDh, `Machine::file_load_ems`): a file from GLOBAL.BSA, one of the
save's, two in a row, and a name there is no file of.

Usage: gen_fileloadems.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The game started and set up as gen_programstart.py has it (GLOBAL.BSA
open, the EMS banks' table made), then for each case from that state:
each name written at DGROUP 1044 and 0000:7428 far-called with SI 1044,
DI FFFDh. One line a case:

  load | NAME | FILES | <dx:ax carry ...> | <runs> | <digest> | <mapped> <ems>

the DX:AX and carry each load returned, what the loads changed in DGROUP
(the open file's name's segment, A90A, taken 1000h lower as the port's),
the bank left mapped (8FA8) and the 32 EMS banks' digest.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_DX, UC_X86_REG_EFLAGS
from emu import fnv
from screens import ScreenArena
from gen_msgbox import runs

CASES = [
    ('bsa', [b'CLOUD01.IMG']),
    ('save', [b'CITYDATA.01']),
    ('two', [b'CLOUD01.IMG', b'CLOUD02.IMG']),
    ('missing', [b'NOSUCH.IMG']),
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
    for name, names in CASES:
        a.restore(snap)
        d0 = bytearray(a.seg_bytes(a.ds))
        got = []
        for n in names:
            a.mu.mem_write(a.lin(a.ds, 0x1044), n + b'\0')
            assert a.run_screen(0, 0x7428, dict(si=0x1044, di=0xFFFD)) is None, f'{name}: the load waited'
            dx, ax = a.mu.reg_read(UC_X86_REG_DX), a.mu.reg_read(UC_X86_REG_AX)
            carry = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
            got.append(f'{dx:x}:{ax:x} {carry}')
        d1 = bytearray(a.seg_bytes(a.ds))
        for d in (d0, d1):
            if struct.unpack_from('<H', d, 0xA90A)[0] == a.ds:
                struct.pack_into('<H', d, 0xA90A, a.ds - 0x1000)
        offs, digest = runs(d0, d1)
        ems = fnv(b''.join(a.ems_bank(b) for b in range(0x20)))
        files = ','.join(n.decode() for n in names)
        out.write(f'load | {name} | {files} | {" ".join(got)} | {offs} | {digest} | {a.r16(0x8FA8):x} {ems:x}\n')
        out.flush()
    out.close()


if __name__ == '__main__':
    main()
