"""Arena emulator with a minimal DOS file layer, for code that does I/O.

int 21h open (3D), read (3F), seek (42) and close (3E) are served from
the user's game directory, matching names case-insensitively, as DOS
would. Everything else that raises an interrupt is a fault, as in
emu.py. The game's own heap runs unmodified once `init_heap` has set it
up (src/heap.rs ports it).
"""
import os
import struct
from emu import Arena, LOAD_SEG
from unicorn.x86_const import *

# Heap paragraphs (absolute, outside the image in the emulator and the
# port alike; the stack is at 9000h).
HEAP_START, HEAP_TOP = 0xA000, 0xF000


class DosArena(Arena):
    def __init__(self, exe_path, game_dir):
        super().__init__(exe_path)
        self.game_dir = game_dir
        self.names = {n.upper(): n for n in os.listdir(game_dir)}
        self.handles = {}

    def init_heap(self, start=HEAP_START, top=HEAP_TOP):
        """Startup's heap setup (0000:00CC): one free block start..top."""
        self.w16(0x159, start)
        self.w16(0x15D, top)
        self.call_near(0, 0x00CC)

    def patch(self, seg, off, code):
        """Replace code at image seg:off; call before running anything."""
        self.mu.mem_write(self.lin(LOAD_SEG + seg, off), code)

    # --- interrupts -------------------------------------------------------

    def _intr(self, mu, intno, _):
        if intno == 0x21 and self._dos():
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

    # --- snapshots ----------------------------------------------------------

    def snapshot(self):
        return bytes(self.mu.mem_read(0, 0x110000)), dict((h, list(f)) for h, f in self.handles.items())

    def restore(self, snap):
        self.mu.mem_write(0, snap[0])
        self.handles = dict((h, list(f)) for h, f in snap[1].items())
