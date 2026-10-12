"""Arena emulator with a minimal DOS file layer, for code that does I/O.

int 21h open (3D), read (3F), seek (42) and close (3E) are served from
the user's game directory, matching names case-insensitively, as DOS
would. The game's heap allocator (0000:51F2) is replaced by a first-fit
allocator that hands out zeroed blocks; the free routines (0000:6C26,
6C32) return blocks to it. Everything else that raises an interrupt is
a fault, as in emu.py.
"""
import os
import struct
from emu import Arena, LOAD_SEG
from unicorn.x86_const import *

HEAP_START, HEAP_END = 0xA000, 0x10F00  # paragraphs (emulator linear / 16)


class DosArena(Arena):
    def __init__(self, exe_path, game_dir):
        super().__init__(exe_path)
        self.game_dir = game_dir
        self.names = {n.upper(): n for n in os.listdir(game_dir)}
        self.handles = {}
        # Allocator stubs; patched before any code runs (unicorn caches
        # translated code, so patches must not come later).
        self.mu.mem_write(self.lin(LOAD_SEG, 0x51F2), b'\xcd\xf1\xc3')  # alloc AX paras -> DX
        self.mu.mem_write(self.lin(LOAD_SEG, 0x6C26), b'\xcd\xf2\xcb')  # free(far ptr) cdecl
        self.mu.mem_write(self.lin(LOAD_SEG, 0x6C32), b'\xcd\xf3\xcb')  # free(AX = segment)
        self.reset_heap()

    def reset_heap(self):
        self.free_list = [(HEAP_START, HEAP_END - HEAP_START)]
        self.used = {}

    def patch(self, seg, off, code):
        """Replace code at image seg:off; call before running anything."""
        self.mu.mem_write(self.lin(LOAD_SEG + seg, off), code)

    # --- interrupts -------------------------------------------------------

    def _intr(self, mu, intno, _):
        if intno == 0x21 and self._dos():
            return
        if intno == 0xF1:
            self._alloc()
            return
        if intno in (0xF2, 0xF3):
            self._free(intno)
            return
        self.fault = intno
        mu.emu_stop()

    def _set_cf(self, cf):
        f = self.mu.reg_read(UC_X86_REG_EFLAGS)
        self.mu.reg_write(UC_X86_REG_EFLAGS, (f | 1) if cf else (f & ~1))

    def _reg(self, r):
        return self.mu.reg_read(globals()['UC_X86_REG_' + r.upper()])

    def _wreg(self, r, v):
        self.mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v & 0xffff)

    def _dos(self):
        ah = self._reg('ax') >> 8
        if ah == 0x3D:
            lin = self.lin(self._reg('ds'), self._reg('dx'))
            raw = bytes(self.mu.mem_read(lin, 128)).split(b'\0')[0].decode('latin1')
            name = self.names.get(raw.upper())
            if name is None:
                self._wreg('ax', 2)
                self._set_cf(True)
                return True
            h = 5
            while h in self.handles:
                h += 1
            self.handles[h] = [open(os.path.join(self.game_dir, name), 'rb').read(), 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        h = self._reg('bx')
        if h not in self.handles:
            return False
        f = self.handles[h]
        if ah == 0x3F:
            n = self._reg('cx')
            data = f[0][f[1]:f[1] + n]
            f[1] += len(data)
            self.mu.mem_write(self.lin(self._reg('ds'), self._reg('dx')), data)
            self._wreg('ax', len(data))
            self._set_cf(False)
            return True
        if ah == 0x42:
            off = (self._reg('cx') << 16) | self._reg('dx')
            al = self._reg('ax') & 0xff
            base = {0: 0, 1: f[1], 2: len(f[0])}[al]
            f[1] = (base + off) & 0xffffffff
            self._wreg('ax', f[1])
            self._wreg('dx', f[1] >> 16)
            self._set_cf(False)
            return True
        if ah == 0x3E:
            del self.handles[h]
            self._set_cf(False)
            return True
        return False

    def _alloc(self):
        paras = self._reg('ax') or 0x10000 >> 4
        for i, (s, n) in enumerate(self.free_list):
            if n >= paras:
                self.free_list[i] = (s + paras, n - paras)
                self.used[s] = paras
                self.mu.mem_write(s * 16, bytes(paras * 16))
                self._wreg('dx', s)
                self._wreg('ax', 0)
                self._set_cf(False)
                return
        raise MemoryError('emulated heap exhausted')

    def _free(self, intno):
        if intno == 0xF2:
            sp = self._reg('sp')
            seg = struct.unpack('<H', self.mu.mem_read(self.lin(self._reg('ss'), sp + 6), 2))[0]
        else:
            seg = self._reg('ax')
        n = self.used.pop(seg, None)
        if n is not None:
            self.free_list.insert(0, (seg, n))
        # Callers here free everything they allocate; starting over when
        # nothing is in use avoids fragmenting this simple allocator.
        if not self.used:
            self.reset_heap()

    # --- snapshots ----------------------------------------------------------

    def snapshot(self):
        return bytes(self.mu.mem_read(0, 0x110000)), dict((h, list(f)) for h, f in self.handles.items())

    def restore(self, snap):
        self.mu.mem_write(0, snap[0])
        self.handles = dict((h, list(f)) for h, f in snap[1].items())
        self.reset_heap()
