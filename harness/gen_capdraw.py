"""Emit test vectors for a square's cap as the 3D mode draws it
(src/render/poly.rs): 0000:CBFF, the cap at height AX on the square A84A,
A84E wound by CX, through B2E5 (BA80 set), FF26/FF74 and the polygon
drawer 7C51 (cut, clipped with INT 0 at 87D4, projected, filled by 8AF2).

Usage: gen_capdraw.py ACD_unpacked.EXE OUT [N]

Each case starts from the same memory: the image's, with EMS off, the
textures' segment ([9030], TSEG) the same 64K of bytes (the LCG stream
from TEXSEED), the remap table's (RSEG) likewise, the page ([A900], PSEG)
EEh, the rows' offsets (A6A0) 140h apart, the view's window and focal
lengths as the game sets them. From the case's seed: the player, the
heading, the eye's height, the view's centre row, the cap's square,
height, winding and texture, and AFEE (0 or RSEG); one case in three
the cap on or by the player's square at the eye's height; three in four
then turned toward the cap's square, give or take 48.

  cap | seed | <page digest> <DGROUP changes digest>
  cap | seed | fault

DGROUP's changes are each byte that differs from the case's start, its
offset (a word) and its new value, leaving out the stack 85AE moves to
(9600h..974Fh) and the caller's SS:SP it keeps (AF8B..AF8E). "fault" where the original
divides past a word outside the clipping (its INT 0 then DOS's).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from gen_attack import fnv, stream
from unicorn.x86_const import *

TSEG = 0x7000
RSEG = 0x6000
PSEG = 0x8000
TEXSEED = 0xCA9_0D7A
REMAPSEED = 0x5EED_0A0E
STACK = (0x9600, 0x9750)
CALLER = (0xAF8B, 0xAF8F)
DS_SIZE = 0x10000


def fixed(a):
    """The memory every case starts from (mirrored in tests/capdraw.rs)."""
    s = stream(TEXSEED)
    a.mu.mem_write(a.lin(TSEG, 0), bytes(next(s) for _ in range(0x10000)))
    s = stream(REMAPSEED)
    a.mu.mem_write(a.lin(RSEG, 0), bytes(next(s) for _ in range(0x100)))
    a.mu.mem_write(a.lin(PSEG, 0), b'\xee' * 0x10000)
    a.mu.mem_write(a.lin(a.ds, 0x8FA6), b'\0')
    a.w16(0x9030, TSEG)
    a.w16(0xA900, PSEG)
    a.mu.mem_write(a.lin(a.ds, 0xBA80), b'\x01')
    for y in range(0xC8):
        a.w16(0xA6A0 + 2 * y, y * 0x140)
    for o, v in ((0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7),
                 (0xC3D6, 0x9F), (0xCCF4, 0xA0), (0xCCF6, 0xA0)):
        a.w16(o, v)


def toward(dx, dz):
    """The heading (0..1FFh, 0 along +z, rising toward +x) roughly toward
    (dx, dz), in integers: an octant and a ratio."""
    ax, az = abs(dx), abs(dz)
    a = ax * 64 // max(az, 1) if ax <= az else 128 - az * 64 // ax
    if dx >= 0:
        return a if dz >= 0 else 256 - a
    return 256 + a if dz < 0 else 512 - a


def setup(seed):
    """(DGROUP words, AX, CX) of case `seed`. Mirrored in
    tests/capdraw.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    col, row = byte() & 0x7F, byte() & 0x7F
    px, pz = (col << 7) + (byte() & 0x7F), (row << 7) + (byte() & 0x7F)
    heading = word() & 0x1FF
    eye = (byte() - 0x40) & 0xFFFF
    centre = 0x30 + byte() % 0x40
    sc = (col + byte() % 7 - 3) & 0x7F
    sr = (row + byte() % 7 - 3) & 0x7F
    height = (word() % 0x400 - 0x200) & 0xFFFF
    wind = byte() & 1
    tex = 1 + byte() % 45
    bank = byte() % 16
    remap = RSEG if byte() % 4 == 0 else 0
    if byte() % 3 == 0:
        # One in three close by, at the eye's height (edges behind the
        # eye: the clipping's divisions that overflow, INT 0 at 87D4).
        sc = (col + byte() % 3 - 1) & 0x7F
        sr = (row + byte() % 3 - 1) & 0x7F
        height = (eye + byte() % 0x41 - 0x20) & 0xFFFF
    if byte() % 4:
        # Three in four turned toward the cap's square, give or take.
        dx = (sc << 7) + 0x40 - px
        dz = (sr << 7) + 0x40 - pz
        heading = (toward(dx, dz) + byte() % 97 - 48) & 0x1FF
    words = [(0xC3D0, px), (0xC3D4, pz), (0xC3DA, heading), (0xC3D2, eye),
             (0xC3D8, centre), (0xA84A, sc), (0xA84E, sr), (0xD804, tex),
             (0xC8F0, bank), (0xAFEE, remap)]
    return words, height, wind


def changes(before, after):
    """Each byte of DGROUP that differs, as (offset word, value), but for
    the stack and the caller's SS:SP."""
    out = bytearray()
    for o in range(DS_SIZE):
        if before[o] != after[o] and not any(lo <= o < hi for lo, hi in (STACK, CALLER)):
            out += struct.pack('<HB', o, after[o])
    return bytes(out)


class CapArena(Arena):
    def _intr(self, mu, intno, _):
        # INT 0 while the clipping has it pointed at 87D4: the handler's
        # 7FFFh signed by DX ^ CX, DX its sign, past the IDIV CX. (Unicorn
        # reports some of these overflows as interrupt 8.)
        if intno in (0, 8) and self.handler_on() and self.at_idiv():
            self.int0()
            return
        super()._intr(mu, intno, _)

    def handler_on(self):
        return struct.unpack('<HH', self.mu.mem_read(0, 4)) == (0x87D4, LOAD_SEG)

    def at_idiv(self):
        mu = self.mu
        cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
        return bytes(mu.mem_read(self.lin(cs, ip), 2)) == b'\xf7\xf9'

    def int0(self):
        mu = self.mu
        cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
        assert bytes(mu.mem_read(self.lin(cs, ip), 2)) == b'\xf7\xf9', 'INT 0 not at an IDIV CX'
        bp = mu.reg_read(UC_X86_REG_BP)
        mu.mem_write(self.lin(LOAD_SEG, 0x87D2), struct.pack('<H', bp))
        dx, cx = mu.reg_read(UC_X86_REG_DX), mu.reg_read(UC_X86_REG_CX)
        m = 0xFFFF if (dx ^ cx) & 0x8000 else 0
        mu.reg_write(UC_X86_REG_AX, ((0x7FFF ^ m) - m) & 0xFFFF)
        mu.reg_write(UC_X86_REG_DX, m)
        mu.reg_write(UC_X86_REG_IP, ip + 2)

    def call(self, ax, cx):
        """0000:CBFF called near; None where the original faults."""
        mu = self.mu
        self._setup([0xFFF0], dict(ax=ax, cx=cx))
        stop = self.lin(LOAD_SEG, 0xFFF0)
        cs_ip = (LOAD_SEG, 0xCBFF)
        steps = 0
        while True:
            self.fault = None
            mu.reg_write(UC_X86_REG_CS, cs_ip[0])
            mu.reg_write(UC_X86_REG_IP, cs_ip[1])
            mu.emu_start(self.lin(*cs_ip), stop, count=20_000_000)
            if self.fault is not None:
                return None
            cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
            if (cs, ip) == (LOAD_SEG, 0xFFF0):
                return True
            # Unicorn's silent stop on a faulting IDIV.
            if bytes(mu.mem_read(self.lin(cs, ip), 2)) == b'\xf7\xf9' and self.handler_on():
                self.int0()
                cs_ip = (cs, mu.reg_read(UC_X86_REG_IP))
                steps += 1
                assert steps < 10000, 'IDIV loop'
                continue
            return None


def main():
    EXE, OUT = sys.argv[1:3]
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 3000
    a = CapArena(EXE)
    fixed(a)
    ds0 = bytes(a.mu.mem_read(a.lin(a.ds, 0), DS_SIZE))
    cs0 = bytes(a.mu.mem_read(a.lin(LOAD_SEG, 0), 0x10000))
    with open(OUT, 'w') as out:
        for i in range(n):
            seed = 0xCA90_0000 + i
            a.mu.mem_write(a.lin(a.ds, 0), ds0)
            a.mu.mem_write(a.lin(LOAD_SEG, 0), cs0)
            a.mu.mem_write(a.lin(PSEG, 0), b'\xee' * 0x10000)
            a.mu.mem_write(0, b'\0' * 4)
            words, ax, cx = setup(seed)
            for o, v in words:
                a.w16(o, v)
            ok = a.call(ax, cx)
            if ok is None:
                out.write(f'cap | {seed:x} | fault\n')
                continue
            page = bytes(a.mu.mem_read(a.lin(PSEG, 0), 0x10000))
            ds = bytes(a.mu.mem_read(a.lin(a.ds, 0), DS_SIZE))
            start = bytearray(ds0)
            for o, v in words:
                struct.pack_into('<H', start, o, v)
            out.write(f'cap | {seed:x} | {fnv(page):x} {fnv(changes(start, ds)):x}\n')


if __name__ == '__main__':
    main()
