"""Emit test vectors for the quests' lines in the equipment list (src/
questitems.rs `quest_item_names`: 1194:A206, then 2433:083D from where
it ended, as 1194:1EB8 calls them).

Usage: gen_questitems.py ACD_unpacked.EXE GAME_DIR OUT [N]

Runs under the DOS file layer (harness/dos.py) with the user's
NAMECHNK.DAT (the persons' names, 1194:0807), GLOBAL.BSA opened first
as the game has it. A case fills from its seed
the four quests at 0DCA (29h bytes each) and the four at 0E6E (34h
each): random bytes, with +4's kind (bits 6-7) mostly 01b, its stage
(bits 9-11) and its bit 20h at random, the race +25h, sex +28h, the
place's kind +27h and the item +6 (and +2Fh) kept within their lists
most of the time; F77 (0..8) and F7F (00h or not); then 126C 0 and
runs A206 with ES:DI DGROUP:1044, and 2433:083D with the DI it left.

  questitems | seed | result

Results: "fault", "hang", or the FNV-1a digests of the changed DGROUP
and DATA (3276) bytes and the AX 083D returns.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_CS, UC_X86_REG_DI, UC_X86_REG_ES, UC_X86_REG_IP
from dos import DosArena
from emu import Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

DATA = 0x3276


def setup(seed):
    """[(offset, bytes)] in DGROUP. Mirrored in tests/questitems.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    w = []
    for base, size, n in ((0x0DCA, 0x29, 4), (0x0E6E, 0x34, 4)):
        for i in range(n):
            r = bytearray(byte() for _ in range(size))
            flags = r[4] | r[5] << 8
            if byte() < 0xD0:
                flags = (flags & ~0xC0) | 0x40
            flags = (flags & ~0xE00) | (byte() % 8) << 9
            struct.pack_into('<H', r, 4, flags)
            if byte() < 0xE0:
                r[0x25] = (r[0x25] & 0x1F) | (byte() % 8) << 5
                r[0x28] = byte() & 1
                r[0x27] = byte() % 8
                r[6] = byte() % 10
                r[7] = 0
                if size == 0x34:
                    r[0x2F] = 0x18 + byte() % 8
                    r[0x30] = 0
            w.append((base + i * size, bytes(r)))
    w.append((0x0F77, bytes([byte() % 9])))
    w.append((0x0F7F, bytes([0 if byte() < 0x80 else 1 + byte() % 0xFF])))
    return w


def main():
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    n = int(sys.argv[4]) if len(sys.argv) > 4 else 3000
    a = DosArena(exe, game)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA (the names file's handle the next)
    # Strings far past a list's end are read from whatever DATA holds
    # there, far pointers among it: as the image has them (segments the
    # image's, as the port's), not as relocated at LOAD_SEG.
    import struct as _s
    raw = open(exe, 'rb').read()
    hdr = _s.unpack('<14H', raw[:28])
    img = raw[hdr[4] * 16:]
    a.mu.mem_write(a.lin(LOAD_SEG + DATA, 0), img[DATA * 16:DATA * 16 + 0x92A0])
    base = a.snapshot()
    with open(out, 'w') as f:
        for k in range(n):
            if not mine(k, n):
                continue
            a.restore(base)
            for at, v in setup(k):
                a.mu.mem_write(a.lin(a.ds, at), v)
            a.w16(0x126C, 0)
            a.w16(0x91F0, 0)
            ds0, d0 = a.seg_bytes(a.ds), a.seg_bytes(LOAD_SEG + DATA)
            try:
                a.fault = None
                a._setup([0xFFF0], {'edi': 0x1044})
                a.mu.reg_write(UC_X86_REG_ES, a.ds)
                a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x1194)
                a.mu.emu_start(a.lin(LOAD_SEG + 0x1194, 0xA206), a.lin(LOAD_SEG + 0x1194, 0xFFF0), count=20_000_000)
                if a.fault is not None:
                    raise Fault(a.fault)
                if a.mu.reg_read(UC_X86_REG_IP) != 0xFFF0:
                    raise TimeoutError
                di = a.mu.reg_read(UC_X86_REG_DI)
                ax = a.call_far(0x2433, 0x083D, edi=di, es=a.ds)
                # The names' substitutions keep DGROUP's segment at A90A:
                # the image's, as the port's (gen_text.py).
                if a.r16(0xA90A) == a.ds:
                    a.w16(0xA90A, a.ds - LOAD_SEG)
                r = '%x %x %x' % (fnv(changes(ds0, a.seg_bytes(a.ds))), fnv(changes(d0, a.seg_bytes(LOAD_SEG + DATA))), ax)
            except (TimeoutError, AssertionError):
                r = 'hang'
            except (Fault, UcError):
                r = 'fault'
            f.write(f'questitems | {k} | {r}\n')


if __name__ == '__main__':
    main()
