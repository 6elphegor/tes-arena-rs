"""Arena emulator with a minimal DOS file layer, for code that does I/O.

int 21h open (3D), read (3F), seek (42) and close (3E) are served from
the user's game directory, matching names case-insensitively, as DOS
would. After `enable_ems`, int 67h 4E02 (the only EMS call 0000:5C8A
makes) maps 64 KB banks into the frame by swapping its bytes with stored
banks, as src/level.rs `ems_map` does. Everything else that raises an interrupt is a fault, as in
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
    def __init__(self, exe_path, game_dir, *more_dirs):
        super().__init__(exe_path)
        self.game_dir = game_dir
        # Paths by upper-case name; later directories win (src/bsa.rs
        # Files::add_dir does the same).
        self.names = {}
        for d in (game_dir,) + more_dirs:
            self.names.update({n.upper(): os.path.join(d, n) for n in os.listdir(d)})
        self.handles = {}
        self.ems = None
        # DOS's handles for the program (its PSP's 20): DOS starts it with
        # its character devices on the first five, CON on 0, 1 and 2 (one
        # open file, one position), AUX on 3, PRN on 4, each open until
        # the program closes it; an open or create takes the lowest free
        # handle. `devices` holds the device handles still open (by
        # handle, the device's name), `dev_pos` each device's position.
        self.devices = {0: 'CON', 1: 'CON', 2: 'CON', 3: 'AUX', 4: 'PRN'}
        self.dev_pos = {'CON': 0, 'AUX': 0, 'PRN': 0}
        # With this set, the devices and closed handles act as in DOS
        # (`_dos_device`). Code that goes on after a failed open uses
        # handle 2, the open's error code ("file not found"), and closes
        # it: from then on 2 is free for the next open, and an operation
        # on 2 with nothing there fails as an invalid handle. Without it,
        # the harness faults on them.
        self.std_devices = False

    def enable_ems(self, frame, banks=0x20, maps_seg=0x0600):
        """EMS on, `banks` 64 KB banks, bank 0 mapped into `frame` (also
        the buffer segment, 9030). The driver's page maps (far pointers at
        DGROUP 9036) are stand-ins holding the bank number."""
        self.w16(0x9030, frame)
        self.mu.mem_write(self.lin(self.ds, 0x8FA6), b'\x01')
        self.w16(0x9032, banks)
        self.w16(0x8FA8, 0)
        for b in range(banks):
            self.w16(b * 2, b, seg=maps_seg)
            self.w16(0x9036 + 4 * b, b * 2)
            self.w16(0x9038 + 4 * b, maps_seg)
        self.ems = {'frame': frame, 'cur': 0, 'banks': {}}

    def ems_bank(self, b):
        """Bank `b` as the program sees it: the frame when mapped."""
        e = self.ems
        if b == e['cur']:
            return self.seg_bytes(e['frame'])
        return e['banks'].get(b, bytes(0x10000))

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
        if intno == 0x67 and self._ems_call():
            return
        super()._intr(mu, intno, _)

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
            # A DOS path is looked up by its last component.
            name = self.names.get(raw.replace(':', '\\').split('\\')[-1].upper())
            if name is None:
                self._wreg('ax', 2)
                self._set_cf(True)
                return True
            h = self.new_handle(name)
            self.handles[h] = [open(name, 'rb').read(), 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        h = self._reg('bx')
        if h not in self.handles:
            return self.std_devices and self._dos_device(ah, h)
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

    def _dos_device(self, ah, h):
        """DOS's read (3Fh), write (40h), seek (42h) or close (3Eh) on a
        handle with no file of the harness's: a device still open, or
        nothing (MS-DOS 4.0's sources, HANDLE.ASM and DISK.ASM).
        - Nothing open: error 6, invalid handle (AX, the carry set; the
          other registers as they were).
        - A read of 0 bytes returns at once, on a device too (DOS_READ's
          "no bytes to read - fast return"). More from CON waits for a
          line typed at the keyboard ($STD_CON_STRING_INPUT): not given
          here, the run faults there (as would a write, which CON would
          show and PRN print).
        - A seek: $LSEEK has no case for devices, the position is the
          device's file's, its size 0.
        - A close frees the handle ($Close frees it even with CON's other
          handles open), AH left 3Eh and AL as it was, as for a file."""
        dev = self.devices.get(h)
        if dev is None:
            if ah not in (0x3E, 0x3F, 0x40, 0x42):
                return False
            self._wreg('ax', 6)
            self._set_cf(True)
            return True
        if ah == 0x3F:
            if self._reg('cx'):
                return False
            self._wreg('ax', 0)
        elif ah == 0x42:
            off = (self._reg('cx') << 16) | self._reg('dx')
            al = self._reg('ax') & 0xff
            base = {0: 0, 1: self.dev_pos[dev]}.get(al, 0)
            p = self.dev_pos[dev] = (base + off) & 0xffffffff
            self._wreg('ax', p)
            self._wreg('dx', p >> 16)
        elif ah == 0x3E:
            del self.devices[h]
        else:
            return False
        self._set_cf(False)
        return True

    def _ems_call(self):
        e = self.ems
        if e is None or self._reg('ax') != 0x4E02:
            return False
        new = self.r16(self._reg('si'), seg=self._reg('ds'))
        self.w16(self._reg('di'), e['cur'], seg=self._reg('es'))
        lin = self.lin(e['frame'], 0)
        e['banks'][e['cur']] = bytes(self.mu.mem_read(lin, 0x10000))
        self.mu.mem_write(lin, e['banks'].get(new, bytes(0x10000)))
        e['cur'] = new
        self._wreg('ax', 0)
        return True

    # --- snapshots ----------------------------------------------------------

    def snapshot(self):
        ems = self.ems and dict(self.ems, banks=dict(self.ems['banks']))
        return (bytes(self.mu.mem_read(0, 0x110000)), dict((h, list(f)) for h, f in self.handles.items()), ems,
                (dict(self.devices), dict(self.dev_pos)))

    def restore(self, snap):
        self.mu.mem_write(0, snap[0])
        self.handles = dict((h, list(f)) for h, f in snap[1].items())
        self.ems = snap[2] and dict(snap[2], banks=dict(snap[2]['banks']))
        self.devices, self.dev_pos = dict(snap[3][0]), dict(snap[3][1])

    def new_handle(self, name, taken=()):
        """The handle an open or create of `name` gets: DOS's lowest free
        (see `devices`), besides those in `taken`."""
        h = 0
        while h in self.handles or h in self.devices or h in taken:
            h += 1
        assert h < 20, 'DOS: too many open files'
        return h
