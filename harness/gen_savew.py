"""Emit test vectors for saving a game (src/save_write.rs: 25FD:0113..01CB
with 0000:A94E) from the original code.

Usage: gen_savew.py ACD_unpacked.EXE GAME_DIR SAVE_DIR SLOT OUT

The saved game in SAVE_DIR (slot SLOT) is loaded by the original's
loaders (SAVEENGN: 25FD:028D..029F, 035A..03F8; SAVEGAME: 0000:A9A6..
AA4A, its three planes allocated first), as gen_save.py does. A case then
changes the state from a seed (`setup`, mirrored in tests/save_write.rs):
bytes of the game state, the level's state, the slots' names, the saved
position, the planes, the slot to save to and the path buffer toggle;
some cases turn EMS on (bank 0 in the buffer segment). The save runs from
25FD:0113 with the slot on the stack. int 21h create, write, close and
delete are served here: the files written are kept in memory. The
palette's read from the VGA (0000:6E3C) and the screen copy and palette
of the load (0000:946A, 9B56) are stubbed.

  savew | seed | result

Results: "fault" / "hang", or for each file written, in order, its name
and the FNV-1a digest of its bytes, then the names deleted, then the
digest of the changed DGROUP bytes. Vectors hold digests only.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_SI

CREATURES, BUFFER, EXTRA = 0x7000, 0x7400, (0x8800, 0x0010)
SCREEN = 0x6000


class WriteDos(DosArena):
    """DosArena with files created, written and deleted in memory."""

    def __init__(self, *a):
        super().__init__(*a)
        self.written = []
        self.deleted = []
        self.open_w = {}

    def _name(self):
        lin = self.lin(self._reg('ds'), self._reg('dx'))
        raw = bytes(self.mu.mem_read(lin, 128)).split(b'\0')[0].decode('latin1')
        return raw.replace(':', '\\').split('\\')[-1].upper()

    def _dos(self):
        ah = self._reg('ax') >> 8
        if ah == 0x3C:
            h = self.new_handle(None, taken=self.open_w)
            self.open_w[h] = [self._name(), bytearray()]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        if ah == 0x41:
            self.deleted.append(self._name())
            self._set_cf(False)
            return True
        h = self._reg('bx')
        if h in self.open_w:
            if ah == 0x40:
                n = self._reg('cx')
                self.open_w[h][1] += bytes(self.mu.mem_read(self.lin(self._reg('ds'), self._reg('dx')), n))
                self._wreg('ax', n)
                self._set_cf(False)
                return True
            if ah == 0x3E:
                self.written.append(tuple(self.open_w.pop(h)))
                self._set_cf(False)
                return True
        return super()._dos()


def setup(seed):
    """(writes, ems, slot): writes are (segment or None for DGROUP,
    offset, bytes). The planes are by index (0 walls, 1 floor, 2 ceiling)
    as negative segments. Mirrored in tests/save_write.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    w = []
    ems = byte() & 1
    slot = byte() % 10
    w.append((None, 0x1A6C, bytes([slot])))
    w.append((None, 0x1AEA, bytes([(0, 0xFF, 1)[byte() % 3]])))
    for _ in range(byte() % 16):
        at = 0x01A4 + word() % 0x0E4F
        w.append((None, at, bytes([byte()])))
    for _ in range(byte() % 16):
        at = 0xBB08 + word() % 0x0DE7
        w.append((None, at, bytes([byte()])))
    for _ in range(byte() % 8):
        at = 0x9686 + word() % 0x01E0
        w.append((None, at, bytes([byte()])))
    if byte() < 0x80:
        w.append((None, 0xBA62, bytes(byte() for _ in range(6))))
    for _ in range(byte() % 8):
        plane = byte() % 3
        at = word() & 0x7FFF
        w.append((-1 - plane, at, bytes([byte()])))
    return w, ems, slot


def main():
    EXE, GAME, SAVE, SLOT, OUT = sys.argv[1:6]
    out = open(OUT, 'w', buffering=1)
    a = WriteDos(EXE, GAME, SAVE)
    for off in (0x946A, 0x9B56, 0x6E3C):
        a.patch(0, off, b'\xc3')
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    # The saved game, loaded as gen_save.py loads it.
    a.w16(0x1042, CREATURES)
    a.w16(0x9030, BUFFER)
    a.w16(0x1020, EXTRA[1])
    a.w16(0x1022, EXTRA[0])
    a.run_range(0x25FD, 0x028D, 0x02A0, eax=int(SLOT))
    a.run_range(0x25FD, 0x035A, 0x03F9)
    a.w16(0xA8FE, 0)
    a.w16(0xA900, SCREEN)
    for var in (0xD806, 0xCCD0, 0xD748):
        r = a.call_near(0, 0x51EC, eax=0x8000)
        a.w16(var, r['dx'])
    a.call_far(0x25FD, 0x0844, esi=0x5E92)
    a.run_range(0, 0xA9A6, 0xAA4B, esi=a.mu.reg_read(UC_X86_REG_SI))
    if a.r16(0xA90A) == a.ds:
        a.w16(0xA90A, a.ds - LOAD_SEG)
    BASE = a.snapshot()
    rng = random.Random(0x1994_25FE)
    total = int(os.environ.get('N', 60))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(BASE)
        a.written, a.deleted, a.open_w = [], [], {}
        writes, ems, slot = setup(seed)
        for seg, off, data in writes:
            if seg is None:
                a.mu.mem_write(a.lin(a.ds, off), data)
            else:
                a.mu.mem_write(a.lin(a.r16((0xD806, 0xCCD0, 0xD748)[-1 - seg]), off), data)
        if ems:
            a.enable_ems(BUFFER)
        before = a.seg_bytes(a.ds)
        try:
            a._setup([slot], {})
            a._run(0x25FD, 0x0113, LOAD_SEG + 0x25FD, 0x01CB)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            parts = [f'{name} {fnv(data):x}' for name, data in a.written]
            parts.append('del ' + ' '.join(a.deleted))
            parts.append(f'{fnv(changes(before, after)):x}')
            res = ' / '.join(parts)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'savew | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
