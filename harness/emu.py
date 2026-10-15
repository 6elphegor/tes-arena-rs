"""Run routines from ACD_unpacked.EXE under unicorn (16-bit real mode)."""
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_16, UC_HOOK_INTR
from unicorn.x86_const import *

LOAD_SEG = 0x1000
DGROUP = 0x3BA0
STUB_SEG = 0x0800  # far return lands here on a HLT


def _fnv_py(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


_FNV_C = b"""
#include <stdint.h>
#include <stddef.h>
uint64_t fnv(const uint8_t *p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < n; i++) h = (h ^ p[i]) * 0x100000001b3ull;
    return h;
}
"""


def _fnv_lib():
    """The C FNV-1a, compiled once into the harness's __pycache__ (None
    without a C compiler: the Python one is used)."""
    import ctypes, hashlib, os, subprocess, tempfile
    d = os.path.join(os.path.dirname(os.path.abspath(__file__)), '__pycache__')
    so = os.path.join(d, 'fnv_%s.so' % hashlib.sha1(_FNV_C).hexdigest()[:12])
    if not os.path.exists(so):
        try:
            os.makedirs(d, exist_ok=True)
            with tempfile.TemporaryDirectory() as t:
                src = os.path.join(t, 'fnv.c')
                open(src, 'wb').write(_FNV_C)
                # Built beside its final name (the same file system), then
                # renamed into place.
                tmp = '%s.%d.tmp' % (so, os.getpid())
                subprocess.run(['cc', '-O2', '-shared', '-fPIC', '-o', tmp, src], check=True, capture_output=True)
                os.replace(tmp, so)
        except (OSError, subprocess.CalledProcessError):
            return None
    lib = ctypes.CDLL(so)
    lib.fnv.restype = ctypes.c_uint64
    lib.fnv.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
    return lib


_FNV = _fnv_lib()


def fnv(b):
    """FNV-1a (64-bit) of the bytes `b` (or an iterable of byte values)."""
    if _FNV is None:
        return _fnv_py(b)
    if not isinstance(b, (bytes, bytearray)):
        b = bytes(b)
    return _FNV.fnv(bytes(b), len(b))


def mine(n, total):
    """Whether case `n` of `total` is this process's: all of them, or with
    SHARD=k/K in the environment the k-th of K contiguous blocks
    (harness/par.py runs the K and joins their outputs in order). Every
    case's seed is drawn either way, so the cases don't change."""
    import os
    shard = os.environ.get('SHARD')
    if not shard:
        return True
    k, K = (int(x) for x in shard.split('/'))
    return total * k // K <= n < total * (k + 1) // K


def changes(old, new):
    """The bytes that differ between two equal-length buffers, as
    (offset word, new byte) records in order: b''.join(struct.pack('<HB',
    i, new[i]) for each i where new[i] != old[i]), comparing 256-byte
    blocks first so that only the blocks that changed are walked."""
    out = []
    for c in range(0, len(new), 256):
        if old[c:c + 256] != new[c:c + 256]:
            for i in range(c, min(c + 256, len(new))):
                if old[i] != new[i]:
                    out.append(struct.pack('<HB', i, new[i]))
    return b''.join(out)


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

    # INT 3s the game itself executes, which DOS's default handler makes
    # no-ops (CS:IP after them, image-relative): 2EBD:0696 in the effect
    # tick and 2CDD:0A1B in the timed-flag lookup. Elsewhere an INT 3 is
    # a harness stub marking a fault.
    GAME_INT3 = {(0x2EBD, 0x0697), (0x2CDD, 0x0A1C)}

    def _intr(self, mu, intno, _):
        if intno == 3:
            at = (mu.reg_read(UC_X86_REG_CS) - LOAD_SEG, mu.reg_read(UC_X86_REG_IP))
            if at in self.GAME_INT3:
                return
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
        """Every call starts from the same state: the return words on a
        fresh stack, the segment registers DGROUP, the general registers 0
        and the flags clear (2, the reserved bit; DF clear), whatever the
        case before left; then `regs`."""
        mu = self.mu
        ss, sp = 0x9000, 0xfff0
        sp -= 2 * len(ret_words)
        mu.mem_write(self.lin(ss, sp), struct.pack(f'<{len(ret_words)}H', *ret_words))
        for r, v in dict(ds=self.ds, es=self.ds, fs=self.ds, gs=self.ds, ss=ss, sp=sp,
                         eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, eflags=2).items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
        for r, v in regs.items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def _run(self, seg, off, stop_seg, stop_off, count=50_000_000):
        mu = self.mu
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
        mu.reg_write(UC_X86_REG_IP, off)
        self.fault = None
        mu.emu_start(self.lin(LOAD_SEG + seg, off), self.lin(stop_seg, stop_off), count=count)
        if self.fault is not None:
            raise Fault(self.fault)
        # Unicorn sometimes stops on a faulting DIV/IDIV without reporting
        # it; F6/F7 with reg field 6 or 7 is DIV/IDIV.
        cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
        if (cs, ip) != (stop_seg, stop_off):
            code = mu.mem_read(self.lin(cs, ip), 8)
            i = 0
            while code[i] in (0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67):  # prefixes
                i += 1
            op, modrm = code[i], code[i + 1]
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
        """Far-call seg:off (image-relative segment) and return AX. From
        the same state as the other calls (`_setup`); a fault the INT hook
        saw, or a DIV unicorn stopped on, raises Fault; not returning
        within 10M instructions fails the assertion (a hang)."""
        self._setup([0, STUB_SEG], regs)
        self._run(seg, off, STUB_SEG, 0, count=10_000_000)
        return self.mu.reg_read(UC_X86_REG_AX)
