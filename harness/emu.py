"""Run routines from ACD_unpacked.EXE under unicorn (16-bit real mode)."""
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16, UC_HOOK_INTR
from unicorn.x86_const import *

LOAD_SEG = 0x1000
DGROUP = 0x3BA0
STUB_SEG = 0x0800  # far return lands here on a HLT


class Fault(Exception):
    """The routine raised a CPU interrupt (0 = divide error) or INT n."""


class Arena:
    def __init__(self, exe_path):
        b = open(exe_path, 'rb').read()
        h = struct.unpack('<14H', b[:28])
        img = bytearray(b[h[4] * 16:])
        for off, seg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]]):
            p = seg * 16 + off
            struct.pack_into('<H', img, p, (struct.unpack_from('<H', img, p)[0] + LOAD_SEG) & 0xffff)
        self.mu = Uc(UC_ARCH_X86, UC_MODE_16)
        self.mu.mem_map(0, 0x110000)
        self.mu.mem_write(LOAD_SEG * 16, bytes(img))
        self.mu.mem_write(STUB_SEG * 16, b'\xf4')
        self.ds = LOAD_SEG + DGROUP
        self.fault = None
        self.mu.hook_add(UC_HOOK_INTR, self._intr)

    def _intr(self, mu, intno, _):
        self.fault = intno
        mu.emu_stop()

    def lin(self, seg, off):
        return seg * 16 + off

    def w16(self, off, v, seg=None):
        self.mu.mem_write(self.lin(self.ds if seg is None else seg, off), struct.pack('<H', v & 0xffff))

    def r16(self, off, seg=None):
        return struct.unpack('<H', self.mu.mem_read(self.lin(self.ds if seg is None else seg, off), 2))[0]

    def w32(self, off, v):
        self.mu.mem_write(self.lin(self.ds, off), struct.pack('<I', v & 0xffffffff))

    def r32(self, off):
        return struct.unpack('<I', self.mu.mem_read(self.lin(self.ds, off), 4))[0]

    def seg_bytes(self, seg):
        return bytes(self.mu.mem_read(self.lin(seg, 0), 0x10000))

    def _setup(self, ret_words, regs):
        mu = self.mu
        ss, sp = 0x9000, 0xfff0
        sp -= 2 * len(ret_words)
        mu.mem_write(self.lin(ss, sp), struct.pack(f'<{len(ret_words)}H', *ret_words))
        for r, v in dict(ds=self.ds, es=self.ds, fs=self.ds, gs=self.ds, ss=ss, sp=sp,
                         eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0).items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
        for r, v in regs.items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def _run(self, seg, off, stop_seg, stop_off):
        mu = self.mu
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
        mu.reg_write(UC_X86_REG_IP, off)
        self.fault = None
        mu.emu_start(self.lin(LOAD_SEG + seg, off), self.lin(stop_seg, stop_off), count=50_000_000)
        if self.fault is not None:
            raise Fault(self.fault)
        # Unicorn sometimes stops on a faulting DIV/IDIV without reporting
        # it; F6/F7 with reg field 6 or 7 is DIV/IDIV.
        cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
        if (cs, ip) != (stop_seg, stop_off):
            op, modrm = mu.mem_read(self.lin(cs, ip), 2)
            if op in (0xF6, 0xF7) and (modrm >> 3) & 7 in (6, 7):
                raise Fault(0)
        assert (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)) == (stop_seg, stop_off), 'did not return'
        return {r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()]) for r in ('ax', 'bx', 'cx', 'dx')}

    def call_near(self, seg, off, **regs):
        """Near-call seg:off; returns registers after its RET."""
        self._setup([0xfff0], regs)
        return self._run(seg, off, LOAD_SEG + seg, 0xfff0)

    def run_range(self, seg, start, end, **regs):
        """Execute seg:start until execution reaches seg:end."""
        self._setup([], regs)
        return self._run(seg, start, LOAD_SEG + seg, end)

    def call_far(self, seg, off, **regs):
        """Far-call seg:off (image-relative segment) and return AX."""
        mu = self.mu
        ss, sp = 0x9000, 0xfff0
        sp -= 4
        mu.mem_write(self.lin(ss, sp), struct.pack('<HH', 0, STUB_SEG))
        for r, v in dict(ds=self.ds, es=self.ds, ss=ss, sp=sp).items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
        for r, v in regs.items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
        mu.reg_write(UC_X86_REG_IP, off)
        self.fault = None
        mu.emu_start(self.lin(LOAD_SEG + seg, off), self.lin(STUB_SEG, 0), count=10_000_000)
        assert mu.reg_read(UC_X86_REG_CS) == STUB_SEG, 'did not return'
        return mu.reg_read(UC_X86_REG_AX)
