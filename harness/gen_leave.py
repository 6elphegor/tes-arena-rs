"""Emit test vectors for leaving an interior (src/progression.rs
`staff_leave`, 1194:D55F; src/loot_window.rs `piles_forget`, 2433:0199)
from the original code.

Usage: gen_leave.py ACD_unpacked.EXE GAME_DIR OUT

  leave | F76 F79 F7C F7E | the patterns deleted (SI at each call of
          1194:4D91, stubbed) | DGROUP's changes (offset:byte)
  forget | piles (FF2) ems AX fill | FNV-1a digests of DGROUP's changes,
           bank 6 and the frame (or "stuck")

The piles' bank (6) is filled with (i x 31 + fill) & FFh, in EMS (on,
20h banks, bank 0 mapped) or, with EMS off, in the frame itself (9030 =
A000h).
"""
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG, changes, fnv
from dos import DosArena
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_SI

FRAME = 0xA000


def bank_fill(fill):
    return bytes((i * 31 + fill) & 0xFF for i in range(0x10000))


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    mu = a.mu
    stub = a.lin(LOAD_SEG + 0x1194, 0x4D91)
    mu.mem_write(stub, b'\xcb')
    deleted = []
    mu.hook_add(UC_HOOK_CODE, lambda mu, *_: deleted.append(mu.reg_read(UC_X86_REG_SI)), begin=stub, end=stub)
    a.enable_ems(FRAME)
    snap = a.snapshot()
    rng = random.Random(0x1994_D55F)
    byte = lambda: rng.choice([0, 0, 1, rng.getrandbits(8)])
    for _ in range(int(os.environ.get('N', 400))):
        a.restore(snap)
        flags = [byte() for _ in range(4)]
        for off, v in zip((0x0F76, 0x0F79, 0x0F7C, 0x0F7E), flags):
            mu.mem_write(a.lin(a.ds, off), bytes([v]))
        before = a.seg_bytes(a.ds)
        deleted.clear()
        a.call_far(0x1194, 0xD55F)
        diff = changes(before, a.seg_bytes(a.ds))
        d = ' '.join(f'{diff[i] | diff[i + 1] << 8:x}:{diff[i + 2]:x}' for i in range(0, len(diff), 3))
        out.write(f"leave | {' '.join(f'{v:x}' for v in flags)} | {' '.join(f'{s:x}' for s in deleted)} | {d}\n")
    for _ in range(int(os.environ.get('N', 400))):
        a.restore(snap)
        piles = rng.choice([0, 1, rng.randint(1, 200), rng.getrandbits(8)])
        ems = rng.random() < 0.6
        ax = rng.choice([0, 6, rng.getrandbits(16)])
        fill = rng.getrandbits(8)
        mu.mem_write(a.lin(a.ds, 0x0FF2), bytes([piles]))
        bank = bank_fill(fill)
        if ems:
            a.ems['banks'][6] = bank
        else:
            mu.mem_write(a.lin(a.ds, 0x8FA6), b'\x00')
            mu.mem_write(a.lin(FRAME, 0), bank)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x2433, 0x0199, eax=ax)
            res = ' '.join(f'{d:x}' for d in (fnv(changes(before, a.seg_bytes(a.ds))), fnv(a.ems_bank(6)),
                                               fnv(a.seg_bytes(FRAME))))
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'forget | {piles:x} {int(ems)} {ax:x} {fill:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
